import type { CodeScanDomain, ScanCategory } from "@/lib/types";
import { CODE_SCAN_DOMAIN_ORDER } from "@/lib/code-scan-domains";
import { SEVERITIES, type Severity } from "@/lib/severity";
import { CATEGORY_ORDER } from "@/lib/tokens";

export type AppTargetPage =
  | "dashboard"
  | "updates"
  | "search-console"
  | "analytics"
  | "integrations"
  | "events"
  | "deploys"
  | "issues"
  | "alerts"
  | "sites"
  | "settings";

const APP_TARGET_PAGES = new Set<AppTargetPage>([
  "dashboard",
  "updates",
  "search-console",
  "analytics",
  "integrations",
  "events",
  "deploys",
  "issues",
  "alerts",
  "sites",
  "settings",
]);

const APP_TARGET_PAGE_ALIASES: Partial<Record<string, AppTargetPage>> = {
  today: "sites",
  scans: "issues",
};

export interface AppTarget {
  page: AppTargetPage;
  projectId?: number | null;
  url?: string | null;
  scanId?: number | null;
  sessionId?: number | null;
  scanKind?: "site" | "code" | null;
  focus?: string | null;
  itemId?: string | null;
  promptId?: string | null;
  lane?: "pending-verification" | null;
  reason?: string | null;
  filePath?: string | null;
  restoreScan?: boolean;
}

export const CODE_SCAN_FOCUS = "code-scan";
export const CODE_SCAN_DOMAIN_FOCUS_PREFIX = "code-scan-domain:";
const ISSUES_STATUS_FOCUS_PREFIX = "issues-status:";
const ISSUES_WEB_CATEGORY_FOCUS_PREFIX = "issues-web-category:";
type IssuesStatusFocus = "active" | "ignored" | "blocked" | "resolved" | "all";

export function normalizeAppTargetPage(value?: string | null): AppTargetPage | null {
  if (!value) return null;
  const aliased = APP_TARGET_PAGE_ALIASES[value] ?? value;
  return APP_TARGET_PAGES.has(aliased as AppTargetPage) ? (aliased as AppTargetPage) : null;
}

export function isCodeScanFocus(value?: string | null): boolean {
  return value === CODE_SCAN_FOCUS || value?.startsWith(CODE_SCAN_DOMAIN_FOCUS_PREFIX) === true;
}

export function getCodeScanDomainFocus(domain: CodeScanDomain): string {
  return `${CODE_SCAN_DOMAIN_FOCUS_PREFIX}${domain}`;
}

export function getCodeScanDomainFromFocus(value?: string | null): CodeScanDomain | null {
  if (!value?.startsWith(CODE_SCAN_DOMAIN_FOCUS_PREFIX)) return null;
  const domain = value.slice(CODE_SCAN_DOMAIN_FOCUS_PREFIX.length) as CodeScanDomain;
  return CODE_SCAN_DOMAIN_ORDER.includes(domain) ? domain : null;
}

export function getIssuesStatusFocus(status: IssuesStatusFocus): string {
  return `${ISSUES_STATUS_FOCUS_PREFIX}${status}`;
}

export function getIssuesStatusFromFocus(value?: string | null): IssuesStatusFocus | null {
  if (!value?.startsWith(ISSUES_STATUS_FOCUS_PREFIX)) return null;
  const status = value.slice(ISSUES_STATUS_FOCUS_PREFIX.length);
  return isIssuesStatusFocus(status) ? status : null;
}

export function getIssuesWebCategoryFocus(category: ScanCategory): string {
  return `${ISSUES_WEB_CATEGORY_FOCUS_PREFIX}${category}`;
}

export function getIssuesWebCategoryFromFocus(value?: string | null): ScanCategory | null {
  if (!value?.startsWith(ISSUES_WEB_CATEGORY_FOCUS_PREFIX)) return null;
  const category = value.slice(ISSUES_WEB_CATEGORY_FOCUS_PREFIX.length) as ScanCategory;
  return CATEGORY_ORDER.includes(category) ? category : null;
}

const ISSUES_SEVERITY_FOCUS_PREFIX = "issues-severity:";
export type IssueSeverityFocus = Severity;

export function getIssuesSeverityFromFocus(value?: string | null): IssueSeverityFocus | null {
  if (!value?.startsWith(ISSUES_SEVERITY_FOCUS_PREFIX)) return null;
  const severity = value.slice(ISSUES_SEVERITY_FOCUS_PREFIX.length) as IssueSeverityFocus;
  return (SEVERITIES as readonly string[]).includes(severity) ? severity : null;
}

function isIssuesStatusFocus(value: string): value is IssuesStatusFocus {
  return (
    value === "active" ||
    value === "ignored" ||
    value === "blocked" ||
    value === "resolved" ||
    value === "all"
  );
}

export function normalizeHttpTargetUrl(value?: string | null): string | null {
  if (!value) return null;
  try {
    const parsed = new URL(value.trim());
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return null;
    if (parsed.username || parsed.password) return null;
    return parsed.toString().replace(/\/$/, "");
  } catch {
    return null;
  }
}

export function normalizeTargetUrl(value?: string | null): string | null {
  return normalizeHttpTargetUrl(value);
}

// Identity keys compare persisted app URLs; input validation still belongs at the boundary.
export function normalizeAppUrlForKey(value?: string | null): string {
  const trimmed = value?.trim();
  if (!trimmed) return "";
  return normalizeHttpTargetUrl(trimmed) ?? trimmed.replace(/\/$/, "");
}

export function normalizeAppUrlForOptionalKey(value?: string | null): string | null {
  return normalizeAppUrlForKey(value) || null;
}

export function withNormalizedTarget(target: AppTarget): AppTarget {
  return {
    ...target,
    page: normalizeAppTargetPage(target.page) ?? target.page,
    url: normalizeTargetUrl(target.url),
    scanId: target.scanId ?? null,
    sessionId: target.sessionId ?? null,
    scanKind: target.scanKind ?? null,
    focus: target.focus ?? null,
    itemId: target.itemId ?? null,
    promptId: target.promptId ?? null,
    lane: target.lane ?? null,
    reason: target.reason ?? null,
    filePath: target.filePath ?? null,
  };
}
