import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  assertChangelogReady,
  evaluateChangelogNotes,
  extractReleaseNotes,
  formatLocalReleaseDate,
  prepareChangelogRelease,
} from "./check-changelog-notes.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

const changelog = (unreleasedBody, tail = "") =>
  `# Changelog\n\nIntro prose.\n\n## [Unreleased]\n${unreleasedBody}${tail}`;

describe("evaluateChangelogNotes", () => {
  it("refuses the scaffold placeholder sentence", () => {
    const verdict = evaluateChangelogNotes(
      changelog(
        "\nNo user-facing changes have been recorded for the initial public release yet.\n",
      ),
    );
    expect(verdict.ok).toBe(false);
    expect(verdict.reason).toContain("no list entries");
  });

  it("refuses an entirely empty Unreleased section", () => {
    expect(evaluateChangelogNotes(changelog("\n\n")).ok).toBe(false);
  });

  it("refuses a file with no Unreleased section at all", () => {
    const verdict = evaluateChangelogNotes("# Changelog\n\n## [1.0.0]\n\n- Something.\n");
    expect(verdict.ok).toBe(false);
    expect(verdict.reason).toContain("[Unreleased]");
  });

  it("does not let entries in a released section below satisfy the check", () => {
    const verdict = evaluateChangelogNotes(
      changelog("\n\n", "\n## [1.0.0]\n\n### Added\n\n- Old, already-released entry.\n"),
    );
    expect(verdict.ok).toBe(false);
  });

  it("passes one dash entry under a category heading", () => {
    expect(evaluateChangelogNotes(changelog("\n### Added\n\n- A real change.\n")).ok).toBe(true);
  });

  it("passes star bullets and indented entries too", () => {
    expect(evaluateChangelogNotes(changelog("\n* A change.\n")).ok).toBe(true);
    expect(evaluateChangelogNotes(changelog("\n  - An indented change.\n")).ok).toBe(true);
  });

  it("does not mistake a horizontal rule or bare dash for an entry", () => {
    expect(evaluateChangelogNotes(changelog("\n---\n")).ok).toBe(false);
    expect(evaluateChangelogNotes(changelog("\n-\n")).ok).toBe(false);
  });
});

describe("assertChangelogReady", () => {
  it("throws with the refusal reason for a placeholder file", () => {
    const dir = fs.mkdtempSync(path.join(ROOT, "tools", "scripts", ".changelog-test-"));
    const file = path.join(dir, "CHANGELOG.md");
    try {
      fs.writeFileSync(file, changelog("\nNothing recorded yet.\n"));
      expect(() => assertChangelogReady({ changelogPath: file })).toThrow(/not ready to release/);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it("throws when the changelog file is missing", () => {
    expect(() =>
      assertChangelogReady({ changelogPath: path.join(ROOT, "no-such-file.md") }),
    ).toThrow(/cannot read/);
  });

  // The live CHANGELOG.md cannot be asserted ready here: immediately after a
  // release roll its Unreleased section is legitimately empty until the next
  // change lands. Release-time readiness is enforced where it matters, by
  // prepareChangelogRelease inside `pnpm release`.
  it("accepts a changelog whose Unreleased section has real entries", () => {
    const dir = fs.mkdtempSync(path.join(ROOT, "tools", "scripts", ".changelog-test-"));
    const file = path.join(dir, "CHANGELOG.md");
    try {
      fs.writeFileSync(file, changelog("\n### Added\n\n- A real change.\n"));
      expect(() => assertChangelogReady({ changelogPath: file })).not.toThrow();
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("prepareChangelogRelease", () => {
  it("uses the maintainer's local calendar date at the UTC day boundary", () => {
    expect(formatLocalReleaseDate(new Date(2026, 7, 17, 23, 59))).toBe("2026-08-17");
  });

  it("freezes Unreleased notes into a dated version and returns the tag body", () => {
    const prepared = prepareChangelogRelease({
      source: changelog("\n### Added\n\n- A real change.\n"),
      version: "1.6.0",
      releaseDate: "2026-08-17",
    });

    expect(prepared.notes).toBe("### Added\n\n- A real change.");
    expect(prepared.source).toContain(
      "## [Unreleased]\n\n## [1.6.0] - 2026-08-17\n\n### Added\n\n- A real change.\n",
    );
    expect(extractReleaseNotes({ source: prepared.source, version: "1.6.0" })).toBe(prepared.notes);
  });

  it("leaves a fresh Unreleased section that blocks a second release without new notes", () => {
    const first = prepareChangelogRelease({
      source: changelog("\n### Added\n\n- First release.\n"),
      version: "1.6.0",
      releaseDate: "2026-08-17",
    });

    expect(evaluateChangelogNotes(first.source).ok).toBe(false);
    expect(() =>
      prepareChangelogRelease({
        source: first.source,
        version: "1.7.0",
        releaseDate: "2026-09-01",
      }),
    ).toThrow(/no list entries/);
  });

  it("preserves older releases below the newly frozen section", () => {
    const prepared = prepareChangelogRelease({
      source: changelog(
        "\n### Fixed\n\n- Current fix.\n",
        "\n## [1.5.4] - 2026-08-01\n\n### Fixed\n\n- Older fix.\n",
      ),
      version: "1.6.0",
      releaseDate: "2026-08-17",
    });

    expect(prepared.source.indexOf("## [1.6.0]")).toBeLessThan(
      prepared.source.indexOf("## [1.5.4]"),
    );
    expect(prepared.source).toContain("- Older fix.");
  });

  it("refuses to create a duplicate release heading", () => {
    expect(() =>
      prepareChangelogRelease({
        source: changelog("\n- New notes.\n", "\n## [1.6.0] - 2026-08-16\n\n- Existing release.\n"),
        version: "1.6.0",
        releaseDate: "2026-08-17",
      }),
    ).toThrow(/already contains/);
  });

  it("refuses to extract notes for an absent or empty released section", () => {
    const source = changelog(
      "\n- New notes.\n",
      "\n## [1.5.4] - 2026-08-01\n\nReleased prose without a list.\n",
    );
    expect(() => extractReleaseNotes({ source, version: "1.6.0" })).toThrow(/no release heading/);
    expect(() => extractReleaseNotes({ source, version: "1.5.4" })).toThrow(/no list entries/);
  });

  it("rejects malformed release versions before creating headings", () => {
    for (const version of ["1.6", "v1.6.0", "1.6.0-", "1.6.0+build"]) {
      expect(() =>
        prepareChangelogRelease({
          source: changelog("\n- Notes.\n"),
          version,
          releaseDate: "2026-08-17",
        }),
      ).toThrow(/invalid changelog release version/);
    }
  });
});
