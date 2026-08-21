import type { ScanType } from "@/lib/types";

export type ScanArtifactType = ScanType | "code" | "session" | string | null | undefined;

export const SCAN_LABELS = {
  web: "Web Scan",
  code: "Code Scan",
  full: "Full Scan",
  multiPageWeb: "Multi-page Web Scan",
} as const;

const WEB_SCAN_SUBTYPE_LABELS: Record<
  Extract<ScanType, "health" | "security" | "accessibility" | "polish">,
  string
> = {
  health: "Full",
  security: "Security",
  accessibility: "Accessibility",
  polish: "Polish",
};

export function getScanArtifactLabel(
  scanType: ScanArtifactType,
  options: { includeHealthSubtype?: boolean } = {},
): string {
  if (scanType === "code") return SCAN_LABELS.code;
  if (scanType === "session") return SCAN_LABELS.multiPageWeb;

  if (scanType && scanType in WEB_SCAN_SUBTYPE_LABELS) {
    if (scanType === "health" && !options.includeHealthSubtype) return SCAN_LABELS.web;
    return `${SCAN_LABELS.web} · ${WEB_SCAN_SUBTYPE_LABELS[scanType as keyof typeof WEB_SCAN_SUBTYPE_LABELS]}`;
  }

  return scanType ? String(scanType) : SCAN_LABELS.web;
}
