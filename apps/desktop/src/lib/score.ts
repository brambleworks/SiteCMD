/** Single source for UI score bands, labels, and color tokens. */

const THRESHOLDS = {
  excellent: 90,
  good: 70,
  attention: 50,
  poor: 30,
} as const;

export const SCORE_CSS_VAR = {
  excellent: "var(--score-excellent)",
  good: "var(--score-good)",
  attention: "var(--score-attention)",
  poor: "var(--score-poor)",
  critical: "var(--score-critical)",
} as const;

export type ScoreBand = "excellent" | "good" | "attention" | "poor" | "critical";

/** Canonical score-band lookup used by labels, styles, and reports. */
export function getScoreBand(score: number): ScoreBand {
  if (score >= THRESHOLDS.excellent) return "excellent";
  if (score >= THRESHOLDS.good) return "good";
  if (score >= THRESHOLDS.attention) return "attention";
  if (score >= THRESHOLDS.poor) return "poor";
  return "critical";
}

/** CSS var reference for SVG fills and inline styles. */
export function getScoreCssVar(score: number): string {
  return SCORE_CSS_VAR[getScoreBand(score)];
}

/** Semantic text-color class for a numeric score (e.g. `text-score-excellent`). */
export function getScoreClass(score: number): string {
  return `text-score-${getScoreBand(score)}`;
}

/** Score to short human label (e.g. "Excellent", "Poor"). */
export function getScoreLabel(score: number): string {
  if (score >= THRESHOLDS.excellent) return "Excellent";
  if (score >= THRESHOLDS.good) return "Good";
  if (score >= THRESHOLDS.attention) return "Needs Attention";
  if (score >= THRESHOLDS.poor) return "Poor";
  return "Critical";
}

/** Returns live-site health guidance for a score. */
export function getScoreContext(score: number): string {
  if (score >= THRESHOLDS.excellent) return "Healthy in production - keep monitoring";
  if (score >= THRESHOLDS.good) return "Mostly healthy - a few fixes remain";
  if (score >= THRESHOLDS.attention) return "Multiple gaps affecting this live site";
  if (score >= THRESHOLDS.poor) return "Significant risk in production - prioritize fixes";
  return "Critical risk live in production - fix urgently";
}

/** Return scan-completion copy with urgency matching the score band. */
export function getScoreMessage(score: number): string {
  if (score >= THRESHOLDS.excellent) return "Looking great!";
  if (score >= THRESHOLDS.good) return "Looking good!";
  if (score >= THRESHOLDS.attention) return "Some issues to address.";
  if (score >= THRESHOLDS.poor) return "Needs attention now.";
  return "Critical - urgent fixes needed.";
}
