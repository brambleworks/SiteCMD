import { severityToneClass } from "@/lib/severity";

export interface CodeScanPresentation {
  title: string;
  scoreLabel: string;
  summaryLabel: string;
  heroCopy: string;
  compareTitle: string;
  compareEmptyTitle: string;
  issuesLabel: string;
  emptyTitle: string;
  emptyCopy: string;
  rerunLabel: string;
  focusLabel: string;
}

export function getCodeScanPresentation(): CodeScanPresentation {
  return {
    title: "Linked project code",
    scoreLabel: "Diagnostic Score",
    summaryLabel: "Code Scan Summary",
    heroCopy:
      "Scans your linked folder for database, security, AI-safety, architecture, and operations risks that public URL scans cannot see.",
    compareTitle: "Compared with previous Code Scan",
    compareEmptyTitle: "This is your first Code Scan for this target",
    issuesLabel: "issues",
    emptyTitle: "No code risks detected",
    emptyCopy: "Your project passed the latest Code Scan.",
    rerunLabel: "Run Code Scan Again",
    focusLabel: "Focus top issue",
  };
}

export const CATEGORY_LABELS: Record<string, string> = {
  security: "Security",
  "ai-safety": "AI Safety",
  "supply-chain": "Dependencies",
  operations: "Operations",
  data: "Database Analysis",
  architecture: "Architecture",
};

export const CATEGORY_ORDER = [
  "data",
  "ai-safety",
  "security",
  "architecture",
  "operations",
  "supply-chain",
];

export const SEVERITY_STYLES: Record<
  string,
  { labelClass: string; tone: "critical" | "warning" | "info" }
> = {
  critical: { labelClass: severityToneClass("critical"), tone: "critical" },
  high: { labelClass: severityToneClass("high"), tone: "warning" },
  medium: { labelClass: severityToneClass("medium"), tone: "info" },
  low: { labelClass: severityToneClass("low"), tone: "info" },
};

export function formatDelta(value: number) {
  if (value > 0) return `+${value}`;
  return String(value);
}
