import { describe, expect, it } from "vitest";
import {
  parseOverrideEntries,
  rangeCapsBelow,
  rangeFloor,
  supplyChainSafetyFailures,
} from "./lib/guardrail-supply-chain-rules.mjs";

const TODAY = new Date("2026-07-24T00:00:00Z");

function workspace(overridesBlock, { minimumReleaseAge = 1440 } = {}) {
  return [`minimumReleaseAge: ${minimumReleaseAge}`, "", "overrides:", overridesBlock, ""].join(
    "\n",
  );
}

// The default graph declares the overridden package under an open range, so a
// case that is only about justification, review date or exact pinning does not
// also trip the "nothing depends on it" check. An empty list means the opposite
// - the package is absent - and is asserted on explicitly below.
function failuresFor(overridesBlock, options = {}) {
  const source = workspace(overridesBlock, options);
  return supplyChainSafetyFailures(() => source, {
    today: TODAY,
    installedRanges: options.installedRanges ?? (() => ["*"]),
  });
}

describe("minimumReleaseAge quarantine", () => {
  it("requires the 1440-minute supply-chain quarantine", () => {
    const failures = failuresFor("", { minimumReleaseAge: 60 });
    expect(failures.some((f) => f.includes("minimumReleaseAge"))).toBe(true);
  });
});

describe("override parsing", () => {
  it("captures the contiguous comment block above each entry", () => {
    const entries = parseOverrideEntries(
      workspace(
        [
          "  # GHSA-aaaa-bbbb-cccc - patched upstream",
          "  # reviewed: 2026-07-24",
          '  sharp: "^0.35.3"',
          "  # non-security: dedupe",
          "  # reviewed: 2026-07-24",
          '  "@scope/pkg": "^2.0.0"',
        ].join("\n"),
      ),
    );
    expect(entries.map((e) => e.name)).toEqual(["sharp", "@scope/pkg"]);
    expect(entries[0].comments.join("\n")).toContain("GHSA-aaaa-bbbb-cccc");
    expect(entries[1].comments.join("\n")).not.toContain("GHSA-aaaa-bbbb-cccc");
  });

  it("stops at the end of the overrides block", () => {
    const source = ["overrides:", '  sharp: "^0.35.3"', "", "allowBuilds:", "  sharp: false"].join(
      "\n",
    );
    expect(parseOverrideEntries(source).map((e) => e.name)).toEqual(["sharp"]);
  });
});

describe("range math", () => {
  it("reads the floor out of common range shapes", () => {
    expect(rangeFloor("^0.35.3")).toEqual([0, 35, 3]);
    expect(rangeFloor("~1.2.3")).toEqual([1, 2, 3]);
    expect(rangeFloor(">=7.24.0")).toEqual([7, 24, 0]);
    expect(rangeFloor("4.12.31")).toEqual([4, 12, 31]);
  });

  it("treats an exact pin below the floor as a cap", () => {
    expect(rangeCapsBelow("0.34.5", [0, 35, 3])).toBe(true);
  });

  it("treats a major-behind caret range as a cap", () => {
    expect(rangeCapsBelow("^1.19.9", [2, 0, 11])).toBe(true);
  });

  it("does not treat an open range as a cap", () => {
    expect(rangeCapsBelow("^8.20.0", [8, 21, 0])).toBe(false);
    expect(rangeCapsBelow("^8.5.16", [8, 5, 10])).toBe(false);
    expect(rangeCapsBelow(">=0.18.0", [0, 28, 0])).toBe(false);
    expect(rangeCapsBelow("*", [0, 28, 0])).toBe(false);
    expect(rangeCapsBelow("7.28.0", [7, 24, 0])).toBe(false);
  });

  it("assumes an unrecognised range caps, so exotic shapes cannot false-fail", () => {
    expect(rangeCapsBelow("next", [1, 0, 0])).toBe(true);
  });

  it("honours a union member that reaches the floor", () => {
    expect(rangeCapsBelow("^0.34.0 || ^0.35.0", [0, 35, 3])).toBe(false);
  });
});

describe("override justification policy", () => {
  const REVIEWED = "  # reviewed: 2026-07-24";

  it("accepts a security override that something in the graph caps below", () => {
    const failures = failuresFor(
      [
        "  # GHSA-f88m-g3jw-g9cj - libvips CVEs inherited by sharp <0.35.0.",
        REVIEWED,
        '  sharp: "^0.35.3"',
      ].join("\n"),
      { installedRanges: () => ["0.34.5", "^0.34.0 || ^0.35.0"] },
    );
    expect(failures).toEqual([]);
  });

  it("rejects an override with no justification at all", () => {
    const failures = failuresFor([REVIEWED, '  postcss: "^8.5.10"'].join("\n"));
    expect(failures.some((f) => f.includes("needs a justification comment"))).toBe(true);
  });

  it("rejects an override with no reviewed date", () => {
    const failures = failuresFor(
      ["  # GHSA-aaaa-bbbb-cccc - something", '  sharp: "^0.35.3"'].join("\n"),
      { installedRanges: () => ["0.34.5"] },
    );
    expect(failures.some((f) => f.includes("reviewed: YYYY-MM-DD"))).toBe(true);
  });

  it("rejects an exact pin that is not explicitly justified as frozen", () => {
    const failures = failuresFor(
      ["  # non-security: reproducibility", REVIEWED, '  devalue: "5.8.1"'].join("\n"),
    );
    expect(failures.some((f) => f.includes("pins the exact version"))).toBe(true);
  });

  it("accepts an exact pin carrying a pinned-exact justification", () => {
    const failures = failuresFor(
      [
        "  # non-security: pinned-exact: the vendor ships broken patch releases.",
        REVIEWED,
        '  devalue: "5.8.1"',
      ].join("\n"),
    );
    expect(failures).toEqual([]);
  });

  it("flags a security override that nothing in the graph caps below", () => {
    const failures = failuresFor(
      ["  # GHSA-aaaa-bbbb-cccc - claimed security floor", REVIEWED, '  postcss: "^8.5.10"'].join(
        "\n",
      ),
      { installedRanges: () => ["^8.5.16"] },
    );
    expect(failures.some((f) => f.includes("is inert"))).toBe(true);
  });

  it("exempts a documented non-security dedupe override from the bind check", () => {
    const failures = failuresFor(
      [
        "  # non-security: deduplication - ink and miniflare disagree.",
        REVIEWED,
        '  ws: "^8.21.0"',
      ].join("\n"),
      { installedRanges: () => ["^8.20.0", "8.21.0"] },
    );
    expect(failures).toEqual([]);
  });

  it("rejects a security override for a package nothing depends on", () => {
    const failures = failuresFor(
      ["  # GHSA-aaaa-bbbb-cccc - patched upstream", REVIEWED, '  sharp: "^0.35.3"'].join("\n"),
      { installedRanges: () => [] },
    );
    expect(failures.some((f) => f.includes("nothing in the installed graph depends on"))).toBe(
      true,
    );
  });

  it("rejects a non-security override for a package nothing depends on", () => {
    const failures = failuresFor(
      [
        "  # non-security: deduplication - ink and miniflare disagree.",
        REVIEWED,
        '  ws: "^8.21.0"',
      ].join("\n"),
      { installedRanges: () => [] },
    );
    expect(failures.some((f) => f.includes("nothing in the installed graph depends on"))).toBe(
      true,
    );
  });

  it("skips the bind check when node_modules is absent", () => {
    const failures = failuresFor(
      ["  # GHSA-aaaa-bbbb-cccc - patched upstream", REVIEWED, '  sharp: "^0.35.3"'].join("\n"),
      { installedRanges: () => null },
    );
    expect(failures).toEqual([]);
  });
});

describe("the live pnpm-workspace.yaml", () => {
  it("passes its own policy", async () => {
    const fs = await import("node:fs");
    const path = await import("node:path");
    const url = await import("node:url");
    const root = path.resolve(path.dirname(url.fileURLToPath(import.meta.url)), "../..");
    const failures = supplyChainSafetyFailures(
      (file) => fs.readFileSync(path.join(root, file), "utf8"),
      { root },
    );
    expect(failures).toEqual([]);
  }, 60_000);
});
