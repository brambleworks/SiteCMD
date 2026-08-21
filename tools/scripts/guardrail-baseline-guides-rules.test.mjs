import { describe, expect, it } from "vitest";
import { baselineGuideShapeFailures } from "./lib/guardrail-baseline-guides-rules.mjs";

const FILE = "apps/desktop/src/lib/fix-guides/security.ts";

function harness(files) {
  return {
    read: (file) => files[file] ?? "",
    listFiles: (dir, filter) =>
      Object.keys(files).filter((file) => file.startsWith(`${dir}/`) && (!filter || filter(file))),
  };
}

function guideModule(entries) {
  return `import type { FixGuideEntry } from "./types";\n\nexport const SECURITY_FIX_GUIDES: Record<string, FixGuideEntry> = ${entries};\n`;
}

describe("baselineGuideShapeFailures", () => {
  it("accepts a bounded baseline entry", () => {
    const h = harness({
      [FILE]: guideModule(
        `{
          "security.hsts": {
            effort: "quick",
            effortMinutes: 5,
            default: ["Enable the header at one layer with a short max-age [see docs] first."],
          },
        }`,
      ),
    });
    expect(baselineGuideShapeFailures(h.read, h.listFiles)).toEqual([]);
  });

  it("flags a framework variant block, the literal deep-guide shape", () => {
    const h = harness({
      [FILE]: guideModule(
        `{
          "security.hsts": {
            effort: "quick",
            effortMinutes: 5,
            default: ["Enable the header."],
            frameworks: { next: ["Use next.config headers."] },
          },
        }`,
      ),
    });
    const failures = baselineGuideShapeFailures(h.read, h.listFiles);
    expect(failures.some((f) => /framework variants/.test(f))).toBe(true);
  });

  it("flags an entry that regrows deep-guide step counts", () => {
    const h = harness({
      [FILE]: guideModule(
        `{
          "security.csp": {
            effort: "involved",
            effortMinutes: 30,
            default: ["one", "two", "three"],
          },
        }`,
      ),
    });
    const failures = baselineGuideShapeFailures(h.read, h.listFiles);
    expect(failures.some((f) => /security\.csp has 3 steps/.test(f))).toBe(true);
  });

  it("flags an overlong step, counting brackets inside strings correctly", () => {
    const h = harness({
      [FILE]: guideModule(
        `{
          "security.csp": {
            effort: "involved",
            effortMinutes: 30,
            default: ["${"x[]".repeat(250)}"],
          },
        }`,
      ),
    });
    const failures = baselineGuideShapeFailures(h.read, h.listFiles);
    expect(failures.some((f) => /-character step/.test(f))).toBe(true);
  });

  it("flags fenced code blocks and empty step lists", () => {
    const h = harness({
      [FILE]: guideModule(
        `{
          "security.a": {
            effort: "quick",
            effortMinutes: 5,
            default: ["Add the header:\\n\\\`\\\`\\\`http\\nX: y\\n\\\`\\\`\\\`"],
          },
          "security.b": { effort: "quick", effortMinutes: 5, default: [] },
        }`,
      ),
    });
    const failures = baselineGuideShapeFailures(h.read, h.listFiles);
    expect(failures.some((f) => /fenced code block/.test(f))).toBe(true);
    expect(failures.some((f) => /security\.b has no steps/.test(f))).toBe(true);
  });

  it("fails loudly when a guide module stops being a literal record", () => {
    const h = harness({
      [FILE]: 'export const SECURITY_FIX_GUIDES = buildGuides("security");\n',
    });
    const failures = baselineGuideShapeFailures(h.read, h.listFiles);
    expect(failures.some((f) => /must be a single typed exported record/.test(f))).toBe(true);
  });

  it("rejects executable guide expressions without running them", () => {
    const probe = "__sitecmdBaselineGuideExecutionProbe";
    globalThis[probe] = false;
    try {
      const h = harness({
        [FILE]: guideModule(
          `{
            "security.hsts": (() => {
              globalThis.${probe} = true;
              return {
                effort: "quick",
                effortMinutes: 5,
                default: ["Enable the header."],
              };
            })(),
          }`,
        ),
      });

      const failures = baselineGuideShapeFailures(h.read, h.listFiles);
      expect(globalThis[probe]).toBe(false);
      expect(failures.some((failure) => /object literal/.test(failure))).toBe(true);
    } finally {
      delete globalThis[probe];
    }
  });

  it("ignores index and types files", () => {
    const h = harness({
      "apps/desktop/src/lib/fix-guides/index.ts": "frameworks: anything goes here",
      "apps/desktop/src/lib/fix-guides/types.ts": "frameworks: anything goes here",
    });
    expect(baselineGuideShapeFailures(h.read, h.listFiles)).toEqual([]);
  });
});
