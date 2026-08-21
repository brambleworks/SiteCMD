const SEO_FOCUS_META = {
  "seo.robots": {
    label: "robots.txt",
    patterns: ["robots", "robots_txt"],
  },
  "seo.sitemap": {
    label: "sitemap",
    patterns: ["sitemap"],
  },
  "seo.titles": {
    label: "title tags",
    patterns: ["title"],
  },
  "seo.descriptions": {
    label: "meta descriptions",
    patterns: ["meta_description", "meta-description", "description"],
  },
  "seo.canonical": {
    label: "canonical URLs",
    patterns: ["canonical"],
  },
  "seo.structured_data": {
    label: "structured data",
    patterns: ["structured", "schema", "json_ld", "json-ld"],
  },
  "seo.noindex": {
    label: "indexing directives",
    patterns: ["noindex", "indexability"],
  },
} as const;

type SeoFocus = keyof typeof SEO_FOCUS_META;

export function getSeoFocusLabel(focus: string | null | undefined): string | null {
  if (!focus) return null;
  return SEO_FOCUS_META[focus as SeoFocus]?.label ?? null;
}

function getSeoFocusPatterns(focus: string | null | undefined): readonly string[] | null {
  if (!focus) return null;
  return SEO_FOCUS_META[focus as SeoFocus]?.patterns ?? null;
}

export function matchesSeoFocusText(haystack: string, focus: string | null | undefined): boolean {
  if (!focus) return false;
  const normalizedHaystack = haystack.toLowerCase();
  const patterns = getSeoFocusPatterns(focus);
  if (!patterns) return normalizedHaystack.includes(focus.toLowerCase());
  return patterns.some((pattern) => normalizedHaystack.includes(pattern.toLowerCase()));
}

export function inferSeoFocusFromText(haystack: string): string | null {
  const focus = Object.keys(SEO_FOCUS_META).find((candidate) =>
    matchesSeoFocusText(haystack, candidate),
  );
  return focus ?? null;
}

export function getSeoWatchImpactSentence(): string {
  return "This could affect crawl directives, sitemap coverage, or indexability.";
}
