/** Integration sources that unlock each progressive navigation page. */

/** Nav pages that only exist once their integration is connected. */
export type ProgressiveNavPage = "analytics" | "search-console" | "deploys";

export const PROGRESSIVE_NAV_PAGES: readonly ProgressiveNavPage[] = [
  "analytics",
  "search-console",
  "deploys",
];

/** A progressive page appears when the project has connected any of these sources. */
export const PROGRESSIVE_NAV_INTEGRATIONS: Record<ProgressiveNavPage, readonly string[]> = {
  analytics: ["plausible", "googleanalytics", "cloudflare"],
  "search-console": ["googlesearchconsole", "bingwebmaster"],
  deploys: ["github"],
};

export function isProgressiveNavPage(page: string): page is ProgressiveNavPage {
  return (PROGRESSIVE_NAV_PAGES as readonly string[]).includes(page);
}

/** True when the project has connected a source that feeds this page. */
export function isNavPageConnected(
  page: ProgressiveNavPage,
  enabled: ReadonlySet<string>,
): boolean {
  return PROGRESSIVE_NAV_INTEGRATIONS[page].some((source) => enabled.has(source));
}
