import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { dossierVerifyCopyFailures } from "./lib/guardrail-dossier-copy-rules.mjs";

const CODE_DOSSIER = "apps/desktop/src/components/scan/CodeIssueDossier.tsx";

function harness(files) {
  return {
    read: (file) => files[file] ?? "",
    exists: (path) => path in files || Object.keys(files).some((key) => key.startsWith(`${path}/`)),
    listFiles: (dir, filter) =>
      Object.keys(files).filter((file) => file.startsWith(`${dir}/`) && (!filter || filter(file))),
  };
}

describe("dossierVerifyCopyFailures", () => {
  it("flags a hardcoded string fallback on a per-issue verify hint (?? and ||)", () => {
    const h = harness({
      [CODE_DOSSIER]: 'const a = issue.verifyHint ?? "Run Code Scan after the patch.";',
      "apps/desktop/src/components/scan/Other.tsx":
        'const b = issue.verifyHint || "generic filler";',
    });
    const failures = dossierVerifyCopyFailures(h.read, h.exists, h.listFiles);
    expect(failures.some((f) => /CodeIssueDossier\.tsx.*hardcoded string/.test(f))).toBe(true);
    expect(failures.some((f) => /Other\.tsx.*hardcoded string/.test(f))).toBe(true);
  });

  it("catches the multi-line fallback form (operator on one line, string on the next)", () => {
    const h = harness({
      [CODE_DOSSIER]: '{issue.verifyHint ??\n  "Run Code Scan after the patch."}',
    });
    expect(dossierVerifyCopyFailures(h.read, h.exists, h.listFiles)).toHaveLength(1);
  });

  it("flags a callout wrapping generic prose or a generic fallback variable", () => {
    const h = harness({
      "apps/desktop/src/components/issues/WebIssueDossier.tsx":
        "<DossierVerifyCallout>\n  Run a fresh verification scan after the change.\n</DossierVerifyCallout>",
      "apps/desktop/src/components/issues/WebIssueDossierBody.tsx":
        "<DossierVerifyCallout>{verifyFallback}</DossierVerifyCallout>",
    });
    const failures = dossierVerifyCopyFailures(h.read, h.exists, h.listFiles);
    expect(failures.some((f) => /WebIssueDossier\.tsx.*generic prose/.test(f))).toBe(true);
    expect(failures.some((f) => /WebIssueDossierBody\.tsx.*generic prose/.test(f))).toBe(true);
  });

  it("allows a callout that carries the per-issue verify hint", () => {
    const h = harness({
      [CODE_DOSSIER]:
        "{issue.verifyHint ? <DossierVerifyCallout>{issue.verifyHint}</DossierVerifyCallout> : null}",
    });
    expect(dossierVerifyCopyFailures(h.read, h.exists, h.listFiles)).toEqual([]);
  });

  it("ignores test files and honors the allow-verify-fallback marker", () => {
    const h = harness({
      "apps/desktop/src/components/issues/DossierStandardSections.test.tsx":
        "<DossierVerifyCallout>Run a fresh scan.</DossierVerifyCallout>",
      [CODE_DOSSIER]:
        "<DossierVerifyCallout>{legacyText}</DossierVerifyCallout> {/* allow-verify-fallback */}",
    });
    expect(dossierVerifyCopyFailures(h.read, h.exists, h.listFiles)).toEqual([]);
  });

  it("passes against the real repository (no filler ships in any dossier)", () => {
    const io = realRepoIo();
    expect(dossierVerifyCopyFailures(io.read, io.exists, io.listFiles)).toEqual([]);
  });
});

function realRepoIo() {
  const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
  const read = (rel) => fs.readFileSync(path.join(ROOT, rel), "utf8");
  const exists = (rel) => fs.existsSync(path.join(ROOT, rel));
  const listFiles = (dir, predicate, acc = []) => {
    for (const entry of fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true })) {
      const rel = `${dir}/${entry.name}`;
      if (entry.isDirectory()) listFiles(rel, predicate, acc);
      else if (!predicate || predicate(rel)) acc.push(rel);
    }
    return acc;
  };
  return { read, exists, listFiles };
}
