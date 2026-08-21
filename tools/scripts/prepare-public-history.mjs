#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = fs.realpathSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", ".."),
);
const USAGE =
  "usage: pnpm publication:prepare -- --backup /absolute/path/sitecmd-private-history.bundle [--apply --confirm-rewrite-main]";

function isWithin(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== "..");
}

export function parsePublicationArguments(arguments_) {
  const options = { apply: false, confirmed: false, backup: "" };

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--") continue;
    if (argument === "--apply") {
      options.apply = true;
      continue;
    }
    if (argument === "--confirm-rewrite-main") {
      options.confirmed = true;
      continue;
    }
    if (argument === "--backup") {
      if (options.backup) throw new Error("--backup may be provided only once");
      options.backup = arguments_[index + 1] ?? "";
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}\n${USAGE}`);
  }

  if (!options.backup) throw new Error(USAGE);
  if (options.apply && !options.confirmed) {
    throw new Error("--apply requires --confirm-rewrite-main");
  }
  if (!options.apply && options.confirmed) {
    throw new Error("--confirm-rewrite-main is valid only with --apply");
  }
  return options;
}

export function resolveBackupPath(repositoryRoot, requestedPath) {
  if (!path.isAbsolute(requestedPath)) {
    throw new Error("the backup bundle path must be absolute");
  }
  if (!requestedPath.endsWith(".bundle")) {
    throw new Error("the backup path must end in .bundle");
  }

  const root = fs.realpathSync(repositoryRoot);
  const parent = fs.realpathSync(path.dirname(path.resolve(requestedPath)));
  const backup = path.join(parent, path.basename(path.resolve(requestedPath)));
  if (isWithin(root, backup)) {
    throw new Error("the backup bundle must be outside the repository checkout");
  }
  if (fs.existsSync(backup)) {
    throw new Error(`the backup bundle already exists: ${backup}`);
  }
  return backup;
}

function run(command, args, cwd, { capture = false } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
  if (result.error) {
    throw new Error(`${command} could not start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const detail = capture
      ? result.stderr.trim() || result.stdout.trim() || `exit ${result.status}`
      : `exit ${result.status}`;
    throw new Error(`${command} ${args[0]} failed: ${detail}`);
  }
  return capture ? result.stdout.trim() : "";
}

function git(runCommand, root, args, options = {}) {
  return runCommand("git", args, root, options);
}

function refSnapshot(root, runCommand) {
  return git(runCommand, root, ["for-each-ref", "--format=%(objectname) %(refname)"], {
    capture: true,
  })
    .split("\n")
    .filter(Boolean);
}

function linkedWorktreePaths(root, runCommand) {
  return git(runCommand, root, ["worktree", "list", "--porcelain", "-z"], { capture: true })
    .split("\0")
    .filter((field) => field.startsWith("worktree "))
    .map((field) => field.slice("worktree ".length));
}

function assertLinkedWorktreesClean(root, runCommand) {
  const repositoryRoot = fs.realpathSync(root);
  const dirtyWorktrees = linkedWorktreePaths(root, runCommand).filter((worktree) => {
    if (fs.realpathSync(worktree) === repositoryRoot) return false;
    return Boolean(
      git(runCommand, root, ["-C", worktree, "status", "--porcelain", "--untracked-files=all"], {
        capture: true,
      }),
    );
  });
  if (dirtyWorktrees.length > 0) {
    throw new Error(
      `reconcile dirty linked worktrees before publication: ${dirtyWorktrees.join(", ")}`,
    );
  }
}

function inspectRepository(root, runCommand) {
  const discoveredRoot = fs.realpathSync(
    git(runCommand, root, ["rev-parse", "--show-toplevel"], { capture: true }),
  );
  if (discoveredRoot !== fs.realpathSync(root)) {
    throw new Error(`run the publication helper from the SiteCMD repository: ${root}`);
  }

  const branch = git(runCommand, root, ["symbolic-ref", "--short", "HEAD"], {
    capture: true,
  });
  if (branch !== "main") throw new Error("publication preparation requires the main branch");

  const status = git(runCommand, root, ["status", "--porcelain", "--untracked-files=all"], {
    capture: true,
  });
  if (status) throw new Error("commit the exact approved tree before publication preparation");
  assertLinkedWorktreesClean(root, runCommand);

  const head = git(runCommand, root, ["rev-parse", "HEAD^{commit}"], { capture: true });
  const tree = git(runCommand, root, ["rev-parse", "HEAD^{tree}"], { capture: true });
  const refs = refSnapshot(root, runCommand);
  return {
    head,
    tree,
    refs,
    refCount: refs.length,
    tagCount: refs.filter((ref) => ref.includes(" refs/tags/")).length,
  };
}

function assertBundleCoversRefs(root, runCommand, backup, expectedRefs) {
  const bundledRefs = new Set(
    git(runCommand, root, ["bundle", "list-heads", backup], { capture: true })
      .split("\n")
      .filter(Boolean),
  );
  const missing = expectedRefs.filter((ref) => !bundledRefs.has(ref));
  if (missing.length > 0) {
    throw new Error(`the private bundle omitted local refs: ${missing.join(", ")}`);
  }
  const currentRefs = refSnapshot(root, runCommand);
  if (currentRefs.join("\n") !== expectedRefs.join("\n")) {
    throw new Error("local refs changed while the private bundle was being created");
  }
}

function assertSigningConfiguration(root, runCommand) {
  const email = git(runCommand, root, ["config", "--get", "user.email"], { capture: true });
  const signingKey = git(runCommand, root, ["config", "--get", "user.signingkey"], {
    capture: true,
  });
  const signingFormat = git(runCommand, root, ["config", "--get", "gpg.format"], {
    capture: true,
  });
  if (!email || !signingKey || signingFormat !== "ssh") {
    throw new Error("configure the documented SSH commit-signing identity before cutover");
  }

  const allowedSignersPath = path.join(root, ".github", "allowed-signers");
  const principals = fs
    .readFileSync(allowedSignersPath, "utf8")
    .split("\n")
    .map((line) => line.trim().split(/\s+/u)[0])
    .filter(Boolean);
  if (!principals.includes(email)) {
    throw new Error(`${email} is not listed in .github/allowed-signers`);
  }
}

export function runPublicationChecks(root, runCommand, expectedTree) {
  runCommand(process.execPath, ["tools/scripts/check-publication-hygiene.mjs"], root);

  const indexTree = git(runCommand, root, ["write-tree"], { capture: true });
  if (indexTree !== expectedTree) {
    throw new Error("the Git index changed after the approved tree was inspected");
  }

  const scanRoot = fs.mkdtempSync(path.join(os.tmpdir(), "sitecmd-public-tree-"));
  try {
    git(runCommand, root, ["checkout-index", "--all", `--prefix=${scanRoot}${path.sep}`]);
    const indexTreeAfterExport = git(runCommand, root, ["write-tree"], { capture: true });
    if (indexTreeAfterExport !== expectedTree) {
      throw new Error("the Git index changed while the approved tree was exported");
    }
    runCommand(
      "gitleaks",
      ["dir", "--redact", "--no-banner", "--config=.gitleaks.toml", "."],
      scanRoot,
    );
  } finally {
    fs.rmSync(scanRoot, { recursive: true, force: true });
  }
}

function runCandidateHistoryCheck(root, runCommand, commit) {
  runCommand(process.execPath, ["tools/scripts/check-publication-history.mjs", commit], root);
}

function assertRootCommit(root, runCommand, commit, expectedTree) {
  const tree = git(runCommand, root, ["rev-parse", `${commit}^{tree}`], { capture: true });
  const rootLine = git(runCommand, root, ["rev-list", "--parents", "--max-count=1", commit], {
    capture: true,
  });
  if (tree !== expectedTree || rootLine.split(/\s+/u).length !== 1) {
    throw new Error("the candidate is not an exact-tree root commit");
  }
}

function printPlan(details, backup, apply, write) {
  const mode = apply ? "APPLY" : "DRY RUN";
  write(
    [
      `Publication history preparation: ${mode}`,
      `Current main: ${details.head}`,
      `Approved tree: ${details.tree}`,
      `Private backup: ${backup}`,
      `Local refs: ${details.refCount} (${details.tagCount} tags)`,
      "",
      "The apply mode will create and verify the private bundle, create a signed",
      "root commit for the current tree, and move local main with an expected-old",
      "SHA check. It will not push, delete refs or tags, or change visibility.",
      "",
    ].join("\n"),
  );
}

export function preparePublicHistory({
  root = ROOT,
  options,
  runCommand = run,
  checkPublication = runPublicationChecks,
  checkCandidateHistory = runCandidateHistoryCheck,
  write = (value) => process.stdout.write(value),
}) {
  const backup = resolveBackupPath(root, options.backup);
  const details = inspectRepository(root, runCommand);
  assertSigningConfiguration(root, runCommand);
  checkPublication(root, runCommand, details.tree);
  printPlan(details, backup, options.apply, write);
  if (!options.apply) return { ...details, backup, applied: false };

  git(runCommand, root, ["bundle", "create", backup, "--all", "HEAD"]);
  git(runCommand, root, ["bundle", "verify", backup], { capture: true });
  assertBundleCoversRefs(root, runCommand, backup, details.refs);

  const commit = git(
    runCommand,
    root,
    ["commit-tree", "-S", details.tree, "-m", "Publish SiteCMD source"],
    { capture: true },
  );
  assertRootCommit(root, runCommand, commit, details.tree);
  git(
    runCommand,
    root,
    ["-c", "gpg.ssh.allowedSignersFile=.github/allowed-signers", "verify-commit", commit],
    { capture: true },
  );
  checkCandidateHistory(root, runCommand, commit);

  git(runCommand, root, ["update-ref", "refs/heads/main", commit, details.head]);
  try {
    const updated = inspectRepository(root, runCommand);
    if (updated.head !== commit || updated.tree !== details.tree) {
      throw new Error("main did not move to the exact-tree root commit");
    }
    const count = git(runCommand, root, ["rev-list", "--count", "refs/heads/main"], {
      capture: true,
    });
    if (count !== "1") throw new Error("rewritten main contains more than one commit");
  } catch (error) {
    git(runCommand, root, ["update-ref", "refs/heads/main", details.head, commit]);
    throw new Error(`publication rewrite failed and main was restored: ${error.message}`, {
      cause: error,
    });
  }

  write(
    [
      `Local main now points to signed root commit ${commit}.`,
      `Private history backup verified at ${backup}.`,
      "Old local branches and tags still exist for backup safety. Do not push them.",
      "Review the cutover checklist before any remote operation.",
      "When the remote is ready, the guarded main update is:",
      `git push --force-with-lease=main:${details.head} origin main`,
      "",
    ].join("\n"),
  );
  return { ...details, backup, commit, applied: true };
}

const modulePath = pathToFileURL(path.resolve(process.argv[1] ?? "")).href;
if (import.meta.url === modulePath) {
  try {
    const options = parsePublicationArguments(process.argv.slice(2));
    preparePublicHistory({ options });
  } catch (error) {
    process.stderr.write(`publication:prepare: ${error.message}\n`);
    process.exitCode = 1;
  }
}
