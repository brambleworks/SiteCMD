import type { ScoreBreakdown, ScoreSnapshot } from "@/lib/types";

// Present the Rust-authored breakdown without recomputing the score.

type ScoreTier = "critical" | "high" | "medium" | "low";

interface ScoreDeductionLine {
  tier: ScoreTier;
  label: string;
  points: number;
}

export interface ScoreBreakdownDisplay {
  overall: number;
  base: number;
  deductions: ScoreDeductionLine[];
  hasDeductions: boolean;
  exploitableCapped: boolean;
  floorApplied: boolean;
  capNote: string | null;
  floorNote: string | null;
}

const TIER_LABELS: Record<ScoreTier, string> = {
  critical: "Critical",
  high: "High",
  medium: "Medium",
  low: "Low",
};

const EXPLOITABLE_CAP_NOTE = "Score capped: a confirmed-exploitable critical issue was found.";
const FLOOR_NOTE = "No full-weight critical issue, so a protective floor lifted the score.";

type BreakdownSource = Pick<ScoreSnapshot, "overall" | "exploitableCapped" | "breakdown">;

const EMPTY_BREAKDOWN: ScoreBreakdown = {
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
};

function tierLine(tier: ScoreTier, rawPoints: number): ScoreDeductionLine | null {
  const points = Math.round(rawPoints);
  if (points < 1) return null;
  return { tier, label: TIER_LABELS[tier], points };
}

/** Adapt a score snapshot into the UI breakdown model. */
export function formatScoreBreakdown(source: BreakdownSource): ScoreBreakdownDisplay {
  const breakdown: ScoreBreakdown = source.breakdown ?? EMPTY_BREAKDOWN;
  const deductions = [
    tierLine("critical", breakdown.criticalPoints),
    tierLine("high", breakdown.highPoints),
    tierLine("medium", breakdown.mediumPoints),
    tierLine("low", breakdown.lowPoints),
  ].filter((line): line is ScoreDeductionLine => line !== null);

  const exploitableCapped = Boolean(source.exploitableCapped);
  return {
    overall: Math.round(source.overall),
    base: Math.round(breakdown.base),
    deductions,
    hasDeductions: deductions.length > 0,
    exploitableCapped,
    floorApplied: breakdown.floorApplied,
    capNote: exploitableCapped ? EXPLOITABLE_CAP_NOTE : null,
    floorNote: breakdown.floorApplied ? FLOOR_NOTE : null,
  };
}
