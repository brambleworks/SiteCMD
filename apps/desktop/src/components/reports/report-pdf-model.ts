import type {
  ReportCodeScanSummary as GeneratedCodeScanSummary,
  ReportData as GeneratedReportData,
  ReportIssue as GeneratedReportIssue,
} from "@/generated/ipc-bindings";

// Keep PDF data on the generated Rust-to-TypeScript contract.
export type CodeScanSummary = GeneratedCodeScanSummary;
export type ReportData = GeneratedReportData;
export type ReportIssue = GeneratedReportIssue;

// The PDF renderer cannot resolve CSS variables, so it needs concrete colors.
export {
  pdfScoreColor as scoreColor,
  pdfSeverityColor as severityColor,
} from "./report-pdf-colors";

// Summarizes the unified score by severity, never by evidence source.
export function buildScoreReconciliation(params: {
  siteScore: number;
  categoryCount: number;
}): string {
  const { siteScore, categoryCount } = params;
  const categoryPhrase = categoryCount > 0 ? ` across ${categoryCount} categories` : "";
  return `The SiteCMD Score of ${siteScore} out of 100 is the single health number for this site, computed from every active issue${categoryPhrase} wherever it lives, weighted by severity.`;
}

export function formatReportDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString("en-US", {
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  } catch {
    return iso;
  }
}
