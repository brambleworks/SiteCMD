import { getScoreBand, type ScoreBand } from "@/lib/score";
import { isSeverity, type Severity } from "@/lib/severity";

/** Score-band colors, print-legible variants of the `--score-*` hue families. */
export const PDF_SCORE: Record<ScoreBand, string> = {
  excellent: "#15803d", // green-700   (5.02:1 on white)
  good: "#c2410c", // orange-700       (5.18:1)
  attention: "#b45309", // amber-700   (5.02:1)
  poor: "#e11d48", // rose-600         (4.70:1)
  critical: "#dc2626", // red-600      (4.83:1)
};

/** Severity colors, print-legible variants of the `--severity-*` hue families. */
export const PDF_SEVERITY: Record<Severity, string> = {
  critical: "#dc2626", // red-600      (4.83:1 on white)
  high: "#e11d48", // rose-600         (4.70:1)
  medium: "#c2410c", // orange-700     (5.18:1)
  low: "#15803d", // green-700         (5.02:1)
};

/** Muted text/border color, matching the light-theme `--muted-foreground` gray. */
export const PDF_MUTED = "#4b5563"; // gray-600 (7.56:1 on white)

/** Concrete hex for a numeric score (0-100). */
export function pdfScoreColor(score: number): string {
  return PDF_SCORE[getScoreBand(score)];
}

/** Concrete hex for an issue severity; falls back to muted for unknown values. */
export function pdfSeverityColor(severity: string): string {
  return isSeverity(severity) ? PDF_SEVERITY[severity] : PDF_MUTED;
}
