import type { FixGuideMeta } from "../fix-guide-shared";
import { ACCESSIBILITY_FIX_GUIDES } from "./accessibility";
import { COMPLIANCE_FIX_GUIDES } from "./compliance";
import { CONFIG_FIX_GUIDES } from "./config";
import { PERFORMANCE_FIX_GUIDES } from "./performance";
import { POLISH_FIX_GUIDES } from "./polish";
import { SECURITY_FIX_GUIDES } from "./security";
import { SEO_FIX_GUIDES } from "./seo";
import type { FixGuideEntry } from "./types";

export type { FixEffort } from "../fix-guide-shared";
export { getEffortLabel } from "../fix-guide-shared";

export interface FixGuide extends FixGuideMeta {
  steps: string[];
}

// Bundled guides provide offline fallbacks when catalog content is unavailable.
const FIX_GUIDES: Record<string, FixGuideEntry> = {
  ...SECURITY_FIX_GUIDES,
  ...SEO_FIX_GUIDES,
  ...PERFORMANCE_FIX_GUIDES,
  ...CONFIG_FIX_GUIDES,
  ...COMPLIANCE_FIX_GUIDES,
  ...ACCESSIBILITY_FIX_GUIDES,
  ...POLISH_FIX_GUIDES,
};

/** Exact top-level guide keys, exported for emitted-check parity tests. */
export const FIX_GUIDE_IDS: readonly string[] = Object.freeze(Object.keys(FIX_GUIDES).sort());

export const FIX_GUIDE_ALIASES: Readonly<Record<string, string>> = Object.freeze({
  "config.source_maps": "source-maps-production",
  "performance.timing": "performance.ttfb",
  "security.headers.cross_origin": "security.cross_origin",
  "security.headers.csp": "security.csp",
  "security.headers.hsts": "security.hsts",
  "security.headers.permissions_policy": "security.permissions_policy",
  "security.headers.referrer_policy": "security.referrer_policy",
  "security.headers.x_content_type_options": "security.x_content_type_options",
  "security.headers.x_frame_options": "security.x_frame_options",
  "security.https_enforcement": "security.https",
  "security.server_info.server_header": "security.server_info",
  "security.server_info.x_powered_by": "security.server_info",
  "security.ssl.expiry": "security.ssl",
  "security.ssl.hostname": "security.ssl",
  "security.ssl.chain": "security.ssl",
  "security.ssl.protocol": "security.ssl",
  "security.exposed_files.summary": "security.exposed_files",
  "security.exposed_files.source_secrets": "security.exposed_files",
  "seo.image_alt": "accessibility.image_alt",
  "seo.duplicate_description": "seo.duplicate_meta",
  "seo.duplicate_description_across_pages": "seo.duplicate_meta",
  "seo.duplicate_title": "seo.duplicate_meta",
  "seo.duplicate_title_across_pages": "seo.duplicate_meta",
  "seo.meta_robots_conflicts": "seo.meta_conflicts",
});

/** Retained only so findings stored by older app versions still have guidance. */
export const LEGACY_FIX_GUIDE_IDS: readonly string[] = Object.freeze(["performance.http_requests"]);

/** Maps an emitted check id to the canonical key used by bundled and catalog guides. */
export function normalizeFixGuideKey(checkId: string): string {
  if (checkId.startsWith("polish.")) return checkId.slice("polish.".length);
  return FIX_GUIDE_ALIASES[checkId] ?? checkId;
}

export function getFixGuide(checkId: string): FixGuide | null {
  const normalizedCheckId = normalizeFixGuideKey(checkId);
  let entry = FIX_GUIDES[normalizedCheckId];

  // Prefix fallback for dynamic sub-IDs (e.g. security.cookies.session -> security.cookies)
  if (!entry) {
    const parts = normalizedCheckId.split(".");
    while (parts.length > 1 && !entry) {
      parts.pop();
      entry = FIX_GUIDES[parts.join(".")];
    }
  }

  if (!entry) return null;

  return { effort: entry.effort, effortMinutes: entry.effortMinutes, steps: entry.default };
}
