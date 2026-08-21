import { getSeoWatchImpactSentence } from "@/lib/seo-focus";
import { getSecurityWatchImpactSentence } from "@/lib/security-focus";

export type DesktopWatchPromptPage = "search-console" | "updates" | "issues";

type DesktopWatchPromptReason =
  "changed-dependencies" | "changed-search-file" | "changed-security-file";

const DESKTOP_WATCH_REASON_TO_KINDS: Record<DesktopWatchPromptReason, readonly string[]> = {
  "changed-dependencies": ["dependencies"],
  "changed-search-file": ["robots", "sitemap"],
  "changed-security-file": ["security-headers", "auth-session", "auth-guard", "cors-config"],
};

const PAGE_REASON_FALLBACKS: Record<DesktopWatchPromptPage, DesktopWatchPromptReason> = {
  updates: "changed-dependencies",
  "search-console": "changed-search-file",
  issues: "changed-security-file",
};

export function normalizeDesktopWatchReason(kind: string, page: DesktopWatchPromptPage): string {
  if (kind.startsWith("changed-")) return kind;

  const fallbackReason = PAGE_REASON_FALLBACKS[page];
  if (DESKTOP_WATCH_REASON_TO_KINDS[fallbackReason].includes(kind)) {
    return fallbackReason;
  }

  return kind;
}

export function getDesktopWatchImpactSentenceForReason(options: {
  reason: string;
  page: DesktopWatchPromptPage;
  focus?: string | null;
}): string | null {
  const reason = options.reason.startsWith("changed-")
    ? options.reason
    : normalizeDesktopWatchReason(options.reason, options.page);

  switch (reason) {
    case "changed-dependencies":
      return "This could affect dependency versions, advisories, or downstream risk.";
    case "changed-search-file":
      return getSeoWatchImpactSentence();
    case "changed-security-file":
      return getSecurityWatchImpactSentence(options.focus);
    default:
      return null;
  }
}
