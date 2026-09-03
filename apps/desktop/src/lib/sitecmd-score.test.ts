import { describe, expect, it } from "vitest";
import { scoreIssueImpact, siteCmdScoreModelFromSnapshot } from "./sitecmd-score";

describe("scoreIssueImpact (finding ranking weight, not the site score)", () => {
  it("ranks higher severity above lower severity", () => {
    expect(scoreIssueImpact("critical", "high", "fail")).toBeGreaterThan(
      scoreIssueImpact("high", "high", "fail"),
    );
    expect(scoreIssueImpact("high", "high", "fail")).toBeGreaterThan(
      scoreIssueImpact("medium", "high", "fail"),
    );
  });

  it("applies confidence multipliers consistently", () => {
    expect(
      (["confirmed", "high", "needs_review"] as const).map((confidence) =>
        scoreIssueImpact("medium", confidence, "fail"),
      ),
    ).toEqual([5, 4.25, 2.75]);
  });

  it("halves the weight of a warn versus a fail", () => {
    expect(scoreIssueImpact("medium", "confirmed", "warn")).toBe(2.5);
    expect(scoreIssueImpact("medium", "confirmed", "fail")).toBe(5);
  });

  it("boosts repeated occurrences up to a saturating cap", () => {
    const one = scoreIssueImpact("low", "confirmed", "fail", 1);
    const many = scoreIssueImpact("low", "confirmed", "fail", 10);
    expect(many).toBeGreaterThan(one);
    // Occurrence boost saturates at MAX_OCCURRENCE_BOOSTS (4), so 5 and 10 agree.
    expect(scoreIssueImpact("low", "confirmed", "fail", 5)).toBe(many);
  });
});

describe("siteCmdScoreModelFromSnapshot", () => {
  it("maps the Rust current-score snapshot into the UI model", () => {
    const model = siteCmdScoreModelFromSnapshot({
      overall: 25.6,
      perCategory: { security: 42, performance: 91 },
      criticalCount: 1,
      highCount: 2,
      mediumCount: 3,
      lowCount: 4,
      exploitableCapped: false,
      breakdown: {
        base: 100,
        criticalPoints: 0,
        highPoints: 0,
        mediumPoints: 0,
        lowPoints: 0,
        effCritical: 0,
        effHigh: 0,
        effMedium: 0,
        effLow: 0,
        floorApplied: false,
        ceilingApplied: false,
      },
      computedAt: 1,
    });

    expect(model.sitecmdScore).toBe(26);
    expect(model.totalIssues).toBe(10);
    expect(model.severityTotals).toEqual({ critical: 1, high: 2, medium: 3, low: 4 });
    // The Rust breakdown is adapted onto the model for the explainability surface.
    expect(model.breakdown.overall).toBe(26);
    expect(model.breakdown.exploitableCapped).toBe(false);
    expect(model.breakdown.hasDeductions).toBe(false);
    // Nothing is open in this breakdown, so the 26 is not the open-issue ceiling.
    expect(model.breakdown.ceilingNote).toBeNull();
  });

  it("rounds the overall score to a whole number and carries the cap flag", () => {
    const model = siteCmdScoreModelFromSnapshot({
      overall: 40.4,
      perCategory: {},
      criticalCount: 1,
      highCount: 0,
      mediumCount: 0,
      lowCount: 0,
      exploitableCapped: true,
      breakdown: {
        base: 100,
        criticalPoints: 59,
        highPoints: 0,
        mediumPoints: 0,
        lowPoints: 0,
        effCritical: 1,
        effHigh: 0,
        effMedium: 0,
        effLow: 0,
        floorApplied: false,
        ceilingApplied: false,
      },
      computedAt: 1,
    });
    expect(model.sitecmdScore).toBe(40);
    expect(model.breakdown.exploitableCapped).toBe(true);
    expect(model.breakdown.capNote).toMatch(/capped/i);
    expect(model.breakdown.deductions).toEqual([
      { tier: "critical", label: "Critical", points: 59 },
    ]);
  });
});
