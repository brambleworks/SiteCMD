import { describe, expect, it } from "vitest";

import { buildScoreReconciliation } from "./report-pdf-model";

describe("buildScoreReconciliation (single unified SiteCMD Score)", () => {
  it("explains the single SiteCMD Score with no competing web/code score numbers", () => {
    const text = buildScoreReconciliation({
      siteScore: 72,
      categoryCount: 6,
    });

    expect(text).toContain("SiteCMD Score of 72");
    expect(text).toContain("across 6 categories");
    expect(text).toMatch(/weighted by severity/i);
    // Negative control: no per-source score decomposition in the copy.
    expect(text).not.toMatch(/Web Scan scored/);
    expect(text).not.toMatch(/Code Scan scored/);
  });

  it("drops the category phrase when no categories are present", () => {
    const text = buildScoreReconciliation({
      siteScore: 50,
      categoryCount: 0,
    });

    expect(text).not.toContain("categories");
    expect(text).toContain("SiteCMD Score of 50");
  });
});
