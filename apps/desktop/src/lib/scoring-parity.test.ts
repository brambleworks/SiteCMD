// scoreIssueImpact ranks the issue list; Rust remains authoritative for SiteCMD Score.

import { describe, expect, it } from "vitest";
import { scoreIssueImpact } from "./sitecmd-score";

describe("scoreIssueImpact ranking heuristic (not the site score)", () => {
  it("orders findings strictly by severity", () => {
    expect(scoreIssueImpact("critical", "confirmed", "fail", 1)).toBeGreaterThan(
      scoreIssueImpact("high", "confirmed", "fail", 1),
    );
    expect(scoreIssueImpact("high", "confirmed", "fail", 1)).toBeGreaterThan(
      scoreIssueImpact("medium", "confirmed", "fail", 1),
    );
    expect(scoreIssueImpact("medium", "confirmed", "fail", 1)).toBeGreaterThan(
      scoreIssueImpact("low", "confirmed", "fail", 1),
    );
  });

  it("weighs a Warn below the equivalent Fail", () => {
    for (const severity of ["critical", "high", "medium", "low"] as const) {
      expect(scoreIssueImpact(severity, "confirmed", "warn", 1)).toBeLessThan(
        scoreIssueImpact(severity, "confirmed", "fail", 1),
      );
    }
  });
});
