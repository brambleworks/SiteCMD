#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  realpathSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const INVENTORY_PATH = path.join(ROOT, "THIRD_PARTY_DEPENDENCIES.json");
const LICENSES_PATH = path.join(ROOT, "THIRD_PARTY_LICENSES.txt");
const MAX_COMMAND_OUTPUT = 128 * 1024 * 1024;
const LICENSE_FILE_STEMS = ["license", "licence", "copying", "notice"];

function runJson(command, args) {
  const result = spawnSync(command, args, {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: MAX_COMMAND_OUTPUT,
  });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed:\n${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout);
}

function normalizeText(value) {
  return (
    value
      .replace(/\r\n?/g, "\n")
      .replace(/[\t ]+$/gm, "")
      .trimEnd() + "\n"
  );
}

function repositoryUrl(pkg) {
  if (typeof pkg.repository === "string") return pkg.repository;
  if (typeof pkg.repository?.url === "string") return pkg.repository.url;
  return typeof pkg.homepage === "string" ? pkg.homepage : null;
}

function isLicenseFileName(name) {
  const lower = name.toLowerCase();
  return LICENSE_FILE_STEMS.some((stem) => {
    if (lower === stem) return true;
    if (!lower.startsWith(stem)) return false;
    return [".", "_", "-"].includes(lower.charAt(stem.length));
  });
}

function candidateLicenseFiles(directory, explicitPath) {
  const candidates = new Set();
  if (explicitPath) {
    const resolved = path.resolve(directory, explicitPath);
    if (existsSync(resolved) && statSync(resolved).isFile()) candidates.add(resolved);
  }
  for (const name of readdirSync(directory).sort()) {
    if (!isLicenseFileName(name)) continue;
    const resolved = path.join(directory, name);
    if (statSync(resolved).isFile()) candidates.add(resolved);
  }
  return [...candidates];
}

function inferLicenseExpression(licensePaths) {
  for (const licensePath of licensePaths) {
    const text = readFileSync(licensePath, "utf8");
    if (
      /^MIT License\s/m.test(text) &&
      text.includes("Permission is hereby granted, free of charge") &&
      text.includes('THE SOFTWARE IS PROVIDED "AS IS"')
    ) {
      return "MIT";
    }
  }
  return null;
}

function resolveInstalledPackage(fromDirectory, name) {
  let directory = fromDirectory;
  while (true) {
    const manifest = path.join(directory, "node_modules", ...name.split("/"), "package.json");
    if (existsSync(manifest)) return realpathSync(path.dirname(manifest));
    const parent = path.dirname(directory);
    if (parent === directory) return null;
    directory = parent;
  }
}

function collectNodePackages(manifestRelativePath, scope, packageRecords) {
  const rootDirectory = path.dirname(path.join(ROOT, manifestRelativePath));
  const rootManifest = JSON.parse(readFileSync(path.join(ROOT, manifestRelativePath), "utf8"));
  const visitedPaths = new Set();

  const visit = (fromDirectory, dependencies = {}) => {
    for (const name of Object.keys(dependencies)) {
      const packageDirectory = resolveInstalledPackage(fromDirectory, name);
      if (!packageDirectory || visitedPaths.has(packageDirectory)) continue;
      visitedPaths.add(packageDirectory);
      const manifest = JSON.parse(
        readFileSync(path.join(packageDirectory, "package.json"), "utf8"),
      );
      const key = `npm:${manifest.name}@${manifest.version}`;
      let record = packageRecords.get(key);
      if (!record) {
        const licensePaths = candidateLicenseFiles(packageDirectory, manifest.licenseFile);
        record = {
          ecosystem: "npm",
          name: manifest.name,
          version: manifest.version,
          license: manifest.license ?? inferLicenseExpression(licensePaths),
          repository: repositoryUrl(manifest),
          source: "npm",
          scopes: new Set(),
          licensePaths,
        };
        packageRecords.set(key, record);
      }
      record.scopes.add(scope);
      visit(packageDirectory, { ...manifest.dependencies, ...manifest.optionalDependencies });
    }
  };
  visit(rootDirectory, { ...rootManifest.dependencies, ...rootManifest.optionalDependencies });
}

function collectRustPackages(manifestRelativePath, scope, packageRecords) {
  const metadata = runJson("cargo", [
    "metadata",
    "--locked",
    "--format-version",
    "1",
    "--manifest-path",
    manifestRelativePath,
  ]);
  const packages = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const queue = [metadata.resolve.root];
  const visited = new Set();

  while (queue.length > 0) {
    const id = queue.shift();
    if (!id || visited.has(id)) continue;
    visited.add(id);
    const pkg = packages.get(id);
    const node = nodes.get(id);
    if (!pkg || !node) continue;

    if (pkg.source) {
      const directory = path.dirname(pkg.manifest_path);
      const key = `cargo:${pkg.name}@${pkg.version}:${pkg.source}`;
      let record = packageRecords.get(key);
      if (!record) {
        record = {
          ecosystem: "cargo",
          name: pkg.name,
          version: pkg.version,
          license: pkg.license ?? null,
          repository: pkg.repository ?? null,
          source: pkg.source,
          scopes: new Set(),
          licensePaths: candidateLicenseFiles(directory, pkg.license_file),
        };
        packageRecords.set(key, record);
      }
      record.scopes.add(scope);
    }

    for (const dependency of node.deps) {
      const isProduction =
        dependency.dep_kinds.length === 0 ||
        dependency.dep_kinds.some((kind) => kind.kind !== "dev");
      if (isProduction) queue.push(dependency.pkg);
    }
  }
}

function buildArtifacts() {
  const packageRecords = new Map();
  collectNodePackages("apps/desktop/package.json", "desktop-javascript", packageRecords);
  collectNodePackages("apps/mcp-server/package.json", "bundled-mcp", packageRecords);
  collectRustPackages("apps/desktop/src-tauri/Cargo.toml", "desktop-rust", packageRecords);
  collectRustPackages("apps/desktop/src-tauri/crates/cli/Cargo.toml", "cli-rust", packageRecords);

  const licenseTexts = new Map();
  const packages = [...packageRecords.values()]
    .sort((a, b) =>
      [a.ecosystem, a.name, a.version]
        .join(":")
        .localeCompare([b.ecosystem, b.name, b.version].join(":")),
    )
    .map((record) => {
      const packageId = `${record.ecosystem}:${record.name}@${record.version}`;
      const licenseFiles = record.licensePaths.map((licensePath) => {
        const content = normalizeText(readFileSync(licensePath, "utf8"));
        const sha256 = createHash("sha256").update(content).digest("hex");
        const entry = licenseTexts.get(sha256) ?? {
          content,
          filenames: new Set(),
          packages: new Set(),
        };
        entry.filenames.add(path.basename(licensePath));
        entry.packages.add(packageId);
        licenseTexts.set(sha256, entry);
        return { name: path.basename(licensePath), sha256 };
      });
      return {
        ecosystem: record.ecosystem,
        name: record.name,
        version: record.version,
        license: record.license,
        repository: record.repository,
        source: record.source,
        scopes: [...record.scopes].sort(),
        licenseFiles,
      };
    });

  const inventory = `${JSON.stringify(
    {
      schemaVersion: 1,
      generatedFrom: [
        "pnpm-lock.yaml",
        "apps/desktop/package.json",
        "apps/mcp-server/package.json",
        "apps/desktop/src-tauri/Cargo.lock",
        "apps/desktop/src-tauri/Cargo.toml",
        "apps/desktop/src-tauri/crates/cli/Cargo.toml",
      ],
      packages,
    },
    null,
    2,
  )}\n`;

  const sections = [...licenseTexts.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([sha256, entry]) =>
      [
        "================================================================================",
        `SHA-256: ${sha256}`,
        `Packages: ${[...entry.packages].sort().join(", ")}`,
        `Upstream files: ${[...entry.filenames].sort().join(", ")}`,
        "================================================================================",
        entry.content.trimEnd(),
      ].join("\n"),
    );
  const licenses = [
    "SiteCMD third-party license texts",
    "",
    "Package identities and SPDX expressions are recorded in THIRD_PARTY_DEPENDENCIES.json.",
    "Identical upstream license files are deduplicated by SHA-256 below.",
    "A package without an upstream license file remains identified by its declared expression.",
    "",
    ...sections,
    "",
  ].join("\n");
  return { inventory, licenses };
}

function checkFile(file, expected) {
  return existsSync(file) && readFileSync(file, "utf8") === expected;
}

const artifacts = buildArtifacts();
if (process.argv.includes("--check")) {
  const stale = [
    [INVENTORY_PATH, artifacts.inventory],
    [LICENSES_PATH, artifacts.licenses],
  ]
    .filter(([file, expected]) => !checkFile(file, expected))
    .map(([file]) => path.relative(ROOT, file));
  if (stale.length > 0) {
    throw new Error(
      `Third-party artifacts are stale: ${stale.join(", ")}. Run pnpm legal:generate.`,
    );
  }
  console.log(`Third-party artifacts are current (${artifacts.inventory.length} inventory bytes).`);
} else {
  writeFileSync(INVENTORY_PATH, artifacts.inventory);
  writeFileSync(LICENSES_PATH, artifacts.licenses);
  console.log("Generated THIRD_PARTY_DEPENDENCIES.json and THIRD_PARTY_LICENSES.txt.");
}
