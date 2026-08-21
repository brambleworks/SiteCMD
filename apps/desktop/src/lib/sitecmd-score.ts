import { ISSUE_CONFIDENCE_MULTIPLIER } from "@/lib/issue-confidence";
import type { SeverityCounts } from "@/lib/severity";
import type { IssueConfidence, Severity } from "@/lib/types";
import type { ScoreSnapshot } from "@/lib/types";
import { formatScoreBreakdown, type ScoreBreakdownDisplay } from "@/lib/score-breakdown";

// Rust owns the SiteCMD score. This module adapts snapshots and ranks findings.

type ScoreImpactStatus = "fail" | "warn";

// These ranking weights are not the Rust health-score deduction curve.
const SEVERITY_BASE_PENALTY: Record<Severity, number> = {
  critical: 25,
  high: 12,
  medium: 5,
  low: 1.5,
};

const STATUS_MULTIPLIER: Record<ScoreImpactStatus, number> = {
  fail: 1,
  warn: 0.5,
};

const OCCURRENCE_BOOST = 0.75;
const MAX_OCCURRENCE_BOOSTS = 4;

function roundImpact(value: number): number {
  return Math.round(value * 100) / 100;
}

/** Relative finding weight for list ordering, not a site-score calculation. */
export function scoreIssueImpact(
  severity: Severity,
  confidence: IssueConfidence,
  status: ScoreImpactStatus,
  occurrenceCount = 1,
): number {
  const base =
    (SEVERITY_BASE_PENALTY[severity] ?? SEVERITY_BASE_PENALTY.medium) *
    (STATUS_MULTIPLIER[status] ?? STATUS_MULTIPLIER.fail);
  const confidenceMultiplier =
    ISSUE_CONFIDENCE_MULTIPLIER[confidence] ?? ISSUE_CONFIDENCE_MULTIPLIER.high;
  const occurrenceBoost = Math.min(Math.max(0, occurrenceCount - 1), MAX_OCCURRENCE_BOOSTS);
  return roundImpact((base + occurrenceBoost * OCCURRENCE_BOOST) * confidenceMultiplier);
}

export interface SiteCmdScoreModel {
  sitecmdScore: number;
  totalIssues: number;
  severityTotals: SeverityCounts;
  // Rust-computed explanation data; the frontend does not derive the score.
  breakdown: ScoreBreakdownDisplay;
}

/** Adapt the Rust-authored current-score snapshot into the UI score model. */
export function siteCmdScoreModelFromSnapshot(snapshot: ScoreSnapshot): SiteCmdScoreModel {
  const totalIssues =
    snapshot.criticalCount + snapshot.highCount + snapshot.mediumCount + snapshot.lowCount;
  return {
    sitecmdScore: Math.round(snapshot.overall),
    totalIssues,
    severityTotals: {
      critical: snapshot.criticalCount,
      high: snapshot.highCount,
      medium: snapshot.mediumCount,
      low: snapshot.lowCount,
    },
    breakdown: formatScoreBreakdown(snapshot),
  };
}
