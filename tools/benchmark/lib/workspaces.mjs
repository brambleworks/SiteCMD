import {
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  realpathSync,
  rmSync,
} from "node:fs";
import path from "node:path";

const BENCHMARK_ARMS = new Set(["blind", "categories", "brief"]);
const SAFE_SLUG = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const COMMIT_SHA = /^[0-9a-f]{40}$/;
const REPOSITORY_SEGMENT = /^[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?$/;

function validateSlug(value, label) {
  if (typeof value !== "string" || !SAFE_SLUG.test(value)) {
    throw new Error(`${label} must be a lowercase kebab-case slug`);
  }
}

function canonicalRepositoryUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error("target repo must be a public GitHub HTTPS URL");
  }
  const segments = url.pathname.split("/").filter(Boolean);
  const repository = segments[1]?.replace(/\.git$/, "");
  if (
    url.protocol !== "https:" ||
    url.hostname !== "github.com" ||
    url.username ||
    url.password ||
    url.port ||
    url.search ||
    url.hash ||
    segments.length !== 2 ||
    !REPOSITORY_SEGMENT.test(segments[0] ?? "") ||
    !REPOSITORY_SEGMENT.test(repository ?? "")
  ) {
    throw new Error("target repo must be a public GitHub HTTPS URL");
  }
  return `https://github.com/${segments[0]}/${repository}.git`.toLowerCase();
}

function validateTarget(target) {
  if (!target || typeof target !== "object") throw new Error("each benchmark target is required");
  validateSlug(target.name, "target name");
  canonicalRepositoryUrl(target.repo);
  if (typeof target.ref !== "string" || !COMMIT_SHA.test(target.ref)) {
    throw new Error(`target ${target.name} ref must be a lowercase 40-character commit SHA`);
  }
}

export function validateBenchmarkConfig(config) {
  if (!config || typeof config !== "object") throw new Error("benchmark config is required");
  if (typeof config.model !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(config.model)) {
    throw new Error("model must be a CLI-safe model identifier");
  }
  if (!Number.isInteger(config.maxTurns) || config.maxTurns < 1) {
    throw new Error("maxTurns must be a positive integer");
  }
  if (!Number.isInteger(config.repeats) || config.repeats < 1) {
    throw new Error("repeats must be a positive integer");
  }
  if (!Array.isArray(config.arms) || config.arms.length === 0) {
    throw new Error("at least one benchmark arm is required");
  }
  for (const arm of config.arms) {
    if (!BENCHMARK_ARMS.has(arm)) throw new Error(`unknown benchmark arm: ${arm}`);
  }
  if (new Set(config.arms).size !== config.arms.length) {
    throw new Error("benchmark arms must be unique");
  }
  if (!Array.isArray(config.targets) || config.targets.length === 0) {
    throw new Error("at least one benchmark target is required");
  }
  for (const target of config.targets) validateTarget(target);
  const names = config.targets.map((target) => target.name);
  if (new Set(names).size !== names.length)
    throw new Error("benchmark target names must be unique");
}

function childPath(root, ...segments) {
  const base = path.resolve(root);
  const candidate = path.resolve(base, ...segments);
  if (candidate === base || !candidate.startsWith(`${base}${path.sep}`)) {
    throw new Error(`benchmark path escapes its workspace: ${candidate}`);
  }
  return candidate;
}

function rejectSymlink(value, label) {
  if (existsSync(value) && lstatSync(value).isSymbolicLink()) {
    throw new Error(`${label} must not be a symbolic link: ${value}`);
  }
}

export function ensurePinnedClone({ target, reposRoot, runGit }) {
  validateTarget(target);
  mkdirSync(reposRoot, { recursive: true });
  const root = realpathSync(reposRoot);
  const dest = childPath(root, target.name);
  rejectSymlink(dest, "benchmark clone");

  if (!existsSync(dest)) {
    runGit(
      ["-c", "credential.helper=", "clone", "--no-checkout", "--no-tags", target.repo, dest],
      root,
    );
  }

  const gitDirectory = path.join(dest, ".git");
  if (!existsSync(gitDirectory) || !lstatSync(gitDirectory).isDirectory()) {
    throw new Error(`benchmark clone cache is not a Git checkout: ${dest}`);
  }
  const origin = runGit(["remote", "get-url", "origin"], dest);
  if (canonicalRepositoryUrl(origin) !== canonicalRepositoryUrl(target.repo)) {
    throw new Error(`benchmark clone ${target.name} has a different cached origin`);
  }

  runGit(
    [
      "-c",
      "credential.helper=",
      "fetch",
      "--force",
      "--no-tags",
      "--depth",
      "1",
      "origin",
      target.ref,
    ],
    dest,
  );
  runGit(["checkout", "--detach", "--force", target.ref], dest);
  runGit(["clean", "-ffdqx"], dest);
  const sha = runGit(["rev-parse", "HEAD"], dest).toLowerCase();
  if (sha !== target.ref) {
    throw new Error(`benchmark clone ${target.name} resolved ${sha}, expected ${target.ref}`);
  }
  return { dest, sha };
}

export function createRunRoot(runsRoot) {
  mkdirSync(runsRoot, { recursive: true });
  const root = realpathSync(runsRoot);
  return mkdtempSync(path.join(root, "run-"));
}

export function createRunCopy({ pristine, runRoot, targetName, arm, repeat }) {
  validateSlug(targetName, "target name");
  if (!BENCHMARK_ARMS.has(arm)) throw new Error(`unknown benchmark arm: ${arm}`);
  if (!Number.isInteger(repeat) || repeat < 0)
    throw new Error("repeat must be a non-negative integer");
  const root = realpathSync(runRoot);
  const destination = childPath(root, targetName, `${arm}-${repeat}`);
  if (existsSync(destination)) throw new Error(`benchmark run copy already exists: ${destination}`);
  mkdirSync(path.dirname(destination), { recursive: true });
  cpSync(pristine, destination, { recursive: true });
  return destination;
}

export function removeRunWorkspace(root, workspace) {
  const resolvedRoot = realpathSync(root);
  rejectSymlink(workspace, "benchmark run workspace");
  const canonicalWorkspace = realpathSync(workspace);
  const resolvedWorkspace = childPath(
    resolvedRoot,
    path.relative(resolvedRoot, canonicalWorkspace),
  );
  rmSync(resolvedWorkspace, { recursive: true, force: true });
}
