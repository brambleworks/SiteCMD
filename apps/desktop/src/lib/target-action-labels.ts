import type { AppTargetPage } from "@/lib/app-targets";
import { isCodeScanFocus } from "@/lib/app-targets";

const OPEN_TARGET_REASON_LABELS = {
  "changed-security-file": "Verify Security",
  "changed-search-file": "Verify Search & SEO",
  "changed-dependencies": "Refresh Updates",
  "deploy-regression": "Review Deploy Regression",
  "no-first-scan": "Run First Web Scan",
  "stale-web-scan": "Refresh Web Scan",
  "stale-code-scan": "Refresh Code Scan",
  "scan-after-deploy": "Scan after Deploy",
} as const;

const OPEN_TARGET_PAGE_LABELS: Partial<Record<AppTargetPage, string>> = {
  "search-console": "Open Search & SEO",
  issues: "Open Issues",
  updates: "Open Updates",
  integrations: "Open Integrations",
  sites: "Open Overview",
  events: "Open Activity",
  deploys: "Open Deploys",
  analytics: "Open Traffic",
  settings: "Open Settings",
  dashboard: "Open Dashboard",
} as const;

const TARGET_PAGE_NOUNS: Partial<Record<AppTargetPage, string>> = {
  "search-console": "Search & SEO",
  issues: "Issues",
  updates: "Updates",
  integrations: "Integrations",
  sites: "Overview",
  events: "Activity",
  deploys: "Deploys",
  analytics: "Traffic",
  settings: "Settings",
  dashboard: "Dashboard",
} as const;

export function getReasonTargetLabel(reason?: string | null): string | null {
  if (!reason) return null;
  return OPEN_TARGET_REASON_LABELS[reason as keyof typeof OPEN_TARGET_REASON_LABELS] ?? null;
}

export function getPageTargetLabel(page?: AppTargetPage | null): string | null {
  if (!page) return null;
  return OPEN_TARGET_PAGE_LABELS[page] ?? null;
}

export function getPageTargetNoun(page?: AppTargetPage | null): string | null {
  if (!page) return null;
  return TARGET_PAGE_NOUNS[page] ?? null;
}

export function getTargetSurfaceLabel(
  target?: {
    page?: AppTargetPage | null;
    focus?: string | null;
    scanKind?: string | null;
    scanId?: number | null;
    sessionId?: number | null;
  } | null,
): string | null {
  if (!target?.page) return null;
  if (target.page === "issues") {
    if (target.scanKind === "code" || isCodeScanFocus(target.focus)) {
      return "Code Scan";
    }
    if (target.scanId != null || target.sessionId != null) {
      return "Results";
    }
    return "Issues";
  }
  return getPageTargetNoun(target.page);
}
