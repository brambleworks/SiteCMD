import type { ScoreSnapshot } from "@/lib/types";

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
  /** The open-issue ceiling, not the arithmetic, set the headline. */
  ceilingApplied: boolean;
  capNote: string | null;
  floorNote: string | null;
  ceilingNote: string | null;
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

/**
 * Deductions are shown to a tenth of a point. The lightest group a live score
 * can carry deducts 0.80 points, which whole points would show as "1", a 25
 * percent overstatement of the only thing standing between the site and 100.
 */
function roundPoints(value: number): number {
  return Math.round(value * 10) / 10;
}

function tierLine(tier: ScoreTier, rawPoints: number): ScoreDeductionLine | null {
  const points = roundPoints(rawPoints);
  if (points <= 0) return null;
  return { tier, label: TIER_LABELS[tier], points };
}

function ceilingNoteFor(totalDeducted: number, base: number, overall: number): string {
  return `Open issues deducted ${roundPoints(totalDeducted)} points, which rounds back to ${base}, so the score is held at ${overall} while any issue is open.`;
}

/** Adapt a score snapshot into the UI breakdown model. */
export function formatScoreBreakdown(source: BreakdownSource): ScoreBreakdownDisplay {
  const breakdown = source.breakdown;
  const deductions = [
    tierLine("critical", breakdown.criticalPoints),
    tierLine("high", breakdown.highPoints),
    tierLine("medium", breakdown.mediumPoints),
    tierLine("low", breakdown.lowPoints),
  ].filter((line): line is ScoreDeductionLine => line !== null);

  const totalDeducted =
    breakdown.criticalPoints + breakdown.highPoints + breakdown.mediumPoints + breakdown.lowPoints;
  const overall = Math.round(source.overall);
  const exploitableCapped = Boolean(source.exploitableCapped);
  const base = Math.round(breakdown.base);
  // Rust says whether the open-issue ceiling set the score
  // (`ceiling_applied` in scoring/calculator.rs); the UI never infers it from
  // the numbers. A live snapshot never carries it, because the lightest group
  // it can hold already lands on 99 by arithmetic.
  const ceilingApplied = breakdown.ceilingApplied;

  return {
    overall,
    base,
    deductions,
    hasDeductions: deductions.length > 0,
    exploitableCapped,
    floorApplied: breakdown.floorApplied,
    ceilingApplied,
    capNote: exploitableCapped ? EXPLOITABLE_CAP_NOTE : null,
    floorNote: breakdown.floorApplied ? FLOOR_NOTE : null,
    ceilingNote: ceilingApplied ? ceilingNoteFor(totalDeducted, base, overall) : null,
  };
}
