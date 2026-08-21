import { describe, expect, it } from "vitest";

import {
  getPageTargetLabel,
  getPageTargetNoun,
  getReasonTargetLabel,
  getTargetSurfaceLabel,
} from "./target-action-labels";

describe("target-action-labels", () => {
  it("returns shared reason-driven open labels", () => {
    expect(getReasonTargetLabel("changed-search-file")).toBe("Verify Search & SEO");
    expect(getReasonTargetLabel("changed-security-file")).toBe("Verify Security");
    expect(getReasonTargetLabel("scan-after-deploy")).toBe("Scan after Deploy");
    expect(getReasonTargetLabel("unknown-reason")).toBeNull();
  });

  it("returns shared page-driven fallback labels", () => {
    expect(getPageTargetLabel("search-console")).toBe("Open Search & SEO");
    expect(getPageTargetLabel("updates")).toBe("Open Updates");
    expect(getPageTargetLabel("dashboard")).toBe("Open Dashboard");
    expect(getPageTargetLabel("issues")).toBe("Open Issues");
  });

  it("returns shared page nouns and target surface labels", () => {
    expect(getPageTargetNoun("search-console")).toBe("Search & SEO");
    expect(getPageTargetNoun("issues")).toBe("Issues");
    expect(getTargetSurfaceLabel({ page: "search-console" })).toBe("Search & SEO");
    expect(
      getTargetSurfaceLabel({
        page: "issues",
        focus: "code-scan-domain:database",
      }),
    ).toBe("Code Scan");
    expect(
      getTargetSurfaceLabel({
        page: "issues",
        scanId: 5,
        scanKind: "site",
      }),
    ).toBe("Results");
    expect(getTargetSurfaceLabel({ page: "issues" })).toBe("Issues");
  });
});
