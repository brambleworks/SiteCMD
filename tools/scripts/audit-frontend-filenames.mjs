#!/usr/bin/env node

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const SRC_ROOT = path.join(ROOT, "apps/desktop/src");

const SOURCE_EXT_RE = /\.(ts|tsx)$/;
const DECLARATION_RE = /\.d\.ts$/;
const LOWER_OR_KEBAB_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const PASCAL_RE = /^[A-Z][A-Za-z0-9]*$/;
const HOOK_RE = /^use[A-Z][A-Za-z0-9]*$/;
const TEST_SUFFIXES = [
  ".behavior.test",
  ".render.test",
  ".performance.test",
  ".capture.test",
  ".copilot.test",
  ".navigation.test",
  ".npm-audit.test",
  ".test",
  ".spec",
];

function toRepoPath(filePath) {
  return path.relative(ROOT, filePath).split(path.sep).join("/");
}

function collectSourceFiles(dir, files = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const filePath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      collectSourceFiles(filePath, files);
      continue;
    }
    if (SOURCE_EXT_RE.test(entry.name) && !DECLARATION_RE.test(entry.name)) {
      files.push(filePath);
    }
  }
  return files;
}

function stripSourceExtension(fileName) {
  return fileName.replace(SOURCE_EXT_RE, "");
}

function stripTestSuffix(baseName) {
  return TEST_SUFFIXES.find((suffix) => baseName.endsWith(suffix)) ?? null;
}

function isCompatibleSourceBase(baseName) {
  return LOWER_OR_KEBAB_RE.test(baseName) || PASCAL_RE.test(baseName) || HOOK_RE.test(baseName);
}

function isUiPrimitiveOrSharedComponent(repoPath) {
  return repoPath.startsWith("apps/desktop/src/components/ui/");
}

function exportedHooks(source) {
  return [...source.matchAll(/export\s+(?:function|const)\s+(use[A-Z][A-Za-z0-9_]*)/g)].map(
    (match) => match[1],
  );
}

function auditFile(filePath) {
  const repoPath = toRepoPath(filePath);
  const fileName = path.basename(filePath);
  const ext = path.extname(fileName);
  const baseName = stripSourceExtension(fileName);
  const failures = [];
  const testSuffix = stripTestSuffix(baseName);

  if (repoPath === "apps/desktop/src/main.tsx") {
    return failures;
  }

  if (testSuffix) {
    const sourceBase = baseName.slice(0, -testSuffix.length);
    if (!isCompatibleSourceBase(sourceBase)) {
      failures.push(
        `test files should mirror a PascalCase, usePascalCase, lowercase, or kebab-case source name before "${testSuffix}"`,
      );
    }
    return failures;
  }

  // A hook filename is use + PascalCase; a bare "use" prefix also matches
  // ordinary words ("user-facing-error"), so require the uppercase boundary.
  if (/^use[A-Z]/.test(baseName)) {
    if (!HOOK_RE.test(baseName)) {
      failures.push("hook files must use usePascalCase");
      return failures;
    }

    const hooks = exportedHooks(readFileSync(filePath, "utf8"));
    if (!hooks.includes(baseName)) {
      failures.push(`hook file must export its matching primary hook "${baseName}"`);
    }
    if (hooks.length > 1) {
      failures.push(`hook file should export one hook; found ${hooks.join(", ")}`);
    }
    return failures;
  }

  if (isUiPrimitiveOrSharedComponent(repoPath)) {
    if (!LOWER_OR_KEBAB_RE.test(baseName) && !PASCAL_RE.test(baseName)) {
      failures.push(
        "apps/desktop/src/components/ui files must be lowercase/kebab primitives or PascalCase shared components",
      );
    }
    return failures;
  }

  if (ext === ".tsx" && !PASCAL_RE.test(baseName)) {
    failures.push("feature component files must use PascalCase.tsx");
  }

  if (ext === ".ts" && !LOWER_OR_KEBAB_RE.test(baseName)) {
    failures.push("non-component TypeScript modules must use lowercase or kebab-case");
  }

  return failures;
}

const files = collectSourceFiles(SRC_ROOT);
const violations = files.flatMap((filePath) =>
  auditFile(filePath).map((message) => ({
    file: toRepoPath(filePath),
    message,
  })),
);

if (violations.length > 0) {
  console.error("Frontend filename audit failed:\n");
  for (const violation of violations) {
    console.error(`- ${violation.file}: ${violation.message}`);
  }
  console.error("\nSee docs/engineering/naming-conventions.md for the canonical rules.");
  process.exit(1);
}

console.log(`Frontend filename audit passed (${files.length} files checked).`);
