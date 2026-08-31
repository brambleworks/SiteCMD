// Runtime navigation vocabulary and typed deep-link targets.

export const NAV_PAGES = [
  "dashboard",
  "analytics",
  "issues",
  "alerts",
  "deploys",
  "events",
  "search-console",
  "updates",
  "settings",
  "reports",
  "integrations",
  "sites",
] as const;

export type NavPage = (typeof NAV_PAGES)[number];

export type NavTarget = NavPage | "today" | `${string}:${string}`;

function isNavPage(value: unknown): value is NavPage {
  return typeof value === "string" && (NAV_PAGES as readonly string[]).includes(value);
}

/** Coerce an untrusted string to a NavPage, falling back when it is not one. */
export function toNavPage(value: unknown, fallback: NavPage = "dashboard"): NavPage {
  return isNavPage(value) ? value : fallback;
}
