import { describe, expect, it } from "vitest";

import type { ScoreBreakdown, ScoreSnapshot } from "@/lib/types";
import { formatScoreBreakdown } from "./score-breakdown";

function breakdown(overrides: Partial<ScoreBreakdown> = {}): ScoreBreakdown {
  return {
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
    ...overrides,
  };
}

function snapshot(
  overrides: Partial<Pick<ScoreSnapshot, "overall" | "exploitableCapped" | "breakdown">> = {},
): Pick<ScoreSnapshot, "overall" | "exploitableCapped" | "breakdown"> {
  return {
    overall: 100,
    exploitableCapped: false,
    breakdown: breakdown(),
    ...overrides,
  };
}

describe("formatScoreBreakdown", () => {
  it("lists points lost per tier, critical -> low, keeping sub-point tiers", () => {
    const display = formatScoreBreakdown(
      snapshot({
        overall: 82,
        breakdown: breakdown({
          criticalPoints: 6,
          highPoints: 9,
          mediumPoints: 2.6,
          lowPoints: 0.3,
          effCritical: 1,
          effHigh: 1,
          effMedium: 1,
          effLow: 1,
        }),
      }),
    );

    expect(display.overall).toBe(82);
    expect(display.base).toBe(100);
    expect(display.deductions).toEqual([
      { tier: "critical", label: "Critical", points: 6 },
      { tier: "high", label: "High", points: 9 },
      { tier: "medium", label: "Medium", points: 2.6 },
      { tier: "low", label: "Low", points: 0.3 },
    ]);
    expect(display.hasDeductions).toBe(true);
    expect(display.ceilingApplied).toBe(false);
    expect(display.ceilingNote).toBeNull();
  });

  it("keeps a sub-point deduction visible instead of rounding it away", () => {
    const display = formatScoreBreakdown(
      snapshot({
        overall: 99,
        breakdown: breakdown({ lowPoints: 0.42, effLow: 0.25 }),
      }),
    );

    expect(display.deductions).toEqual([{ tier: "low", label: "Low", points: 0.4 }]);
    expect(display.hasDeductions).toBe(true);
  });

  it("explains a 99 that Rust says the open-issue ceiling held", () => {
    const display = formatScoreBreakdown(
      snapshot({
        overall: 99,
        breakdown: breakdown({ lowPoints: 0.42, effLow: 0.25, ceilingApplied: true }),
      }),
    );

    expect(display.ceilingApplied).toBe(true);
    expect(display.ceilingNote).toContain("0.4 points");
    expect(display.ceilingNote).toContain("rounds back to 100");
    expect(display.ceilingNote).toContain("held at 99");
    expect(display.ceilingNote).toMatch(/open/i);
    expect(display.capNote).toBeNull();
    expect(display.floorNote).toBeNull();
  });

  it("takes the ceiling from the Rust flag rather than the arithmetic", () => {
    // Both shapes below read like a ceiling if you only compare the headline
    // against the deduction lines, and Rust says neither is one.
    const nothingOpen = formatScoreBreakdown(snapshot({ overall: 26 }));
    expect(nothingOpen.ceilingApplied).toBe(false);
    expect(nothingOpen.ceilingNote).toBeNull();

    const alreadyExplained = formatScoreBreakdown(
      snapshot({
        overall: 25,
        breakdown: breakdown({
          criticalPoints: 15,
          highPoints: 17.1,
          effCritical: 1,
          effHigh: 2,
        }),
      }),
    );
    expect(alreadyExplained.ceilingApplied).toBe(false);
    expect(alreadyExplained.ceilingNote).toBeNull();
  });

  it("reports a clean score with no deductions and no notes", () => {
    const display = formatScoreBreakdown(snapshot({ overall: 100 }));

    expect(display.deductions).toEqual([]);
    expect(display.hasDeductions).toBe(false);
    expect(display.capNote).toBeNull();
    expect(display.floorNote).toBeNull();
    expect(display.ceilingApplied).toBe(false);
    expect(display.ceilingNote).toBeNull();
  });

  it("surfaces an honest cap note when the score is exploitable-capped (D6/D7)", () => {
    const display = formatScoreBreakdown(
      snapshot({
        // 100 - 15 = 85 before the cap, so the gap here is the cap's doing.
        overall: 49,
        exploitableCapped: true,
        breakdown: breakdown({ criticalPoints: 15, effCritical: 1 }),
      }),
    );

    expect(display.exploitableCapped).toBe(true);
    expect(display.capNote).toMatch(/capped/i);
    expect(display.capNote).toMatch(/exploitable/i);
    // The exploitable cap, not the open-issue ceiling, owns this gap.
    expect(display.ceilingApplied).toBe(false);
    expect(display.ceilingNote).toBeNull();
  });

  it("surfaces the zero-critical floor note only when the floor applied", () => {
    const lifted = formatScoreBreakdown(
      snapshot({ overall: 40, breakdown: breakdown({ highPoints: 60, floorApplied: true }) }),
    );
    expect(lifted.floorApplied).toBe(true);
    expect(lifted.floorNote).toMatch(/floor/i);

    const notLifted = formatScoreBreakdown(
      snapshot({ overall: 40, breakdown: breakdown({ highPoints: 60, floorApplied: false }) }),
    );
    expect(notLifted.floorNote).toBeNull();
  });

  it("rounds the headline overall like the rest of the UI", () => {
    const display = formatScoreBreakdown(snapshot({ overall: 25.6 }));
    expect(display.overall).toBe(26);
    expect(display.ceilingNote).toBeNull();
  });
});
