import { CODE_SCAN_DOMAIN_ORDER } from "@/lib/code-scan-domains";
import { CATEGORY_LABELS, CATEGORY_ORDER } from "@/lib/tokens";
import type { CodeScanDomain, ScanCategory } from "@/lib/types";

/**
 * One category vocabulary for issues, shared by web checks and code domains.
 * `security` is deliberately a single key: a security finding is one category
 * whether the web scan or the code scan saw it.
 */
export type IssueCategoryKey = ScanCategory | CodeScanDomain;

/**
 * Category names for the code domains. Kept separate from the scan-surface
 * domain metadata so the list reads as categories ("AI Safety", "Operations")
 * rather than lane shorthand ("AI", "Ops").
 */
const CODE_DOMAIN_CATEGORY_LABELS: Record<CodeScanDomain, string> = {
  database: "Database",
  "ai-safety": "AI Safety",
  security: CATEGORY_LABELS.security,
  architecture: "Architecture",
  operations: "Operations",
  "supply-chain": "Dependencies",
  "ai-scaffolding": "AI Setup",
};

const WEB_CATEGORY_KEYS = new Set<string>(CATEGORY_ORDER);

/** Web categories first, then the code domains they do not already cover. */
export const ISSUE_CATEGORY_ORDER: IssueCategoryKey[] = [
  ...CATEGORY_ORDER,
  ...CODE_SCAN_DOMAIN_ORDER.filter((domain) => !WEB_CATEGORY_KEYS.has(domain)),
];

export function issueCategoryLabel(key: IssueCategoryKey | string): string {
  const web = (CATEGORY_LABELS as Record<string, string | undefined>)[key];
  if (web) return web;
  const code = (CODE_DOMAIN_CATEGORY_LABELS as Record<string, string | undefined>)[key];
  if (code) return code;
  return key;
}
