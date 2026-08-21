import { describe, expect, it } from "vitest";
import { getScoreCssVar, getScoreClass, getScoreLabel, getScoreContext } from "./score";

describe("score helpers", () => {
  it("labels excellent for >= 90", () => {
    expect(getScoreLabel(95)).toBe("Excellent");
    expect(getScoreLabel(90)).toBe("Excellent");
  });
  it("labels good for 70-89", () => {
    expect(getScoreLabel(80)).toBe("Good");
    expect(getScoreLabel(70)).toBe("Good");
  });
  it("labels needs attention for 50-69", () => {
    expect(getScoreLabel(60)).toBe("Needs Attention");
  });
  it("css var matches band", () => {
    expect(getScoreCssVar(95)).toContain("excellent");
    expect(getScoreCssVar(40)).toContain("poor");
  });
  it("class matches band", () => {
    expect(getScoreClass(95)).toBe("text-score-excellent");
  });
  it("context provides production-appropriate guidance without launch framing (D4/D10)", () => {
    expect(getScoreContext(95)).toMatch(/production/i);
    expect(getScoreContext(40)).toMatch(/risk/i);
    // No pre-launch framing anywhere in the band ladder.
    for (const score of [95, 75, 55, 40, 10]) {
      expect(getScoreContext(score)).not.toMatch(/launch/i);
    }
  });
});
