const LEGACY_CARD_CLASS_STRINGS = [
  "rounded-xl bg-card p-4 ghost-border",
  "rounded-xl p-4 ghost-border bg-card",
  "bg-card rounded-xl p-4 ghost-border",
  "rounded-xl p-5 ghost-border bg-card",
  "rounded-xl ghost-border p-5 bg-muted",
  "rounded-xl ghost-border p-6 bg-muted",
  "rounded-lg ghost-border bg-card p-5",
];

const LEGACY_SURFACE_CLASS_MARKERS = [
  "card-shell",
  "surface-card",
  "surface-card-loose",
  "surface-mini-card",
  "surface-list-action",
  "dashboard-clickable-card",
  "dashboard-card-title",
  "dashboard-label-icon",
  "dashboard-tile-label",
  "dashboard-tile-rule",
  "dashboard-tile-cta",
  "dashboard-click-row",
  "issue-list-action",
];

const INLINE_STYLE_ALLOWED_FILES = new Set([
  "apps/desktop/src/components/reports/ReportPDFSections.tsx",
  "apps/desktop/src/components/ui/progress-bar.tsx",
  "apps/desktop/src/components/ui/score-ring.tsx",
]);

const STYLE_CLASS_PREFIX =
  /^(card|panel|tile|list-row|empty-state|action-card|metric-card|project-card|integration-card|alert-row|report-preview|row|section|page|body|text-|icon-|btn-|field-|tone-|source-|severity-|status-|meta-|mono-|muted-|dashboard-|scan-|launch-|issue-|dossier-|settings-|activity-|overview-|jobs-|calendar-|popover-|keyboard-|compact-|workflow-|command-|inline-|primary-|success-|warning-|danger-|refresh-|guided-|update-|context-|details-|summary-|error-|filter-|toggle-|tab|tooltip|placeholder|input|select|progress|skeleton|spinner|evidence|instance|code-|unread-|cwv-|obsidian|ghost-border|ghost-border-hover|link-|form-|stat-|queue-|show-|aftership-|disabled-|tiny-|trial-|add-project-|feature-|events-|report-|hero-|loading-|modal-|rail-|score-|search-|security-|analytics-|deploy|owner-|proof-|walkthrough-|kbd-|nav-|topbar-|overlay-|surface-|nested-|attention-|split-|toolbar-|history-|session-|comparison-|date-|event-|segmented-|module-|dashed-|callout-|responsive-|connected-|black-|two-column-|week-|timeline-|engine-|new-|builder-)/;
const UTILITY_CLASS_PREFIX =
  /^(flex|grid|block|inline|hidden|absolute|relative|fixed|sticky|inset|top|right|bottom|left|z-|w-|h-|size-|min-|max-|p[trblxy]?|m[trblxy]?|-m|space-|gap-|items-|justify-|content-|self-|text-|font-|leading-|tracking-|uppercase|lowercase|capitalize|normal-case|truncate|whitespace-|break-|overflow|rounded|border|bg-|shadow|ring|opacity|transition|duration|ease|hover:|focus:|focus-visible:|active:|disabled:|group-|sm:|md:|lg:|xl:|2xl:|cursor-|select-|shrink|grow|basis|object-|aspect-|divide-|backdrop-|pointer-|translate|scale|rotate|animate|tabular|align|order|col-|row-)/;

function hasTooManyRawUtilityClasses(source) {
  const matches = source.matchAll(/className="([^"]+)"/g);
  for (const match of matches) {
    const tokens = match[1].split(/\s+/).filter(Boolean);
    const rawUtilityCount = tokens.filter(
      (token) => UTILITY_CLASS_PREFIX.test(token) && !STYLE_CLASS_PREFIX.test(token),
    ).length;
    if (tokens.length > 5 && rawUtilityCount >= 6) return true;
  }
  return false;
}

export function desktopStyleConsistencyFailures(read, sourceFiles) {
  const failures = [];
  const surfaceViolations = [];
  const inlineStyleViolations = [];
  const rawButtonViolations = [];
  const longClassViolations = [];
  const utilityClusterViolations = [];
  const cvaViolations = [];
  const targetFiles = sourceFiles.filter((file) => file.endsWith(".tsx"));
  const cvaScanFiles = sourceFiles.filter((file) => file.endsWith(".tsx") || file.endsWith(".ts"));
  const styleApiFiles = sourceFiles.filter(
    (file) => file.endsWith(".tsx") || file.endsWith(".css"),
  );

  for (const file of targetFiles) {
    const source = read(file);
    for (const classString of LEGACY_CARD_CLASS_STRINGS) {
      if (source.includes(classString)) {
        surfaceViolations.push(`${file} uses "${classString}"`);
      }
    }
    if (
      source.includes("style={{") &&
      !INLINE_STYLE_ALLOWED_FILES.has(file) &&
      !file.endsWith(".test.tsx")
    ) {
      inlineStyleViolations.push(file);
    }
    if (
      source.includes("<button") &&
      file !== "apps/desktop/src/components/ui/button.tsx" &&
      // Google's branded sign-in control is the only shared primitive allowed
      // to render its mandated raw button.
      file !== "apps/desktop/src/components/ui/google-sign-in-button.tsx" &&
      !file.endsWith(".test.tsx")
    ) {
      rawButtonViolations.push(file);
    }
    const longClassMatches = source.match(/className="[^"]{100,}"/g) ?? [];
    if (longClassMatches.length > 0 && !file.endsWith(".test.tsx")) {
      longClassViolations.push(`${file} (${longClassMatches.length})`);
    }
    if (hasTooManyRawUtilityClasses(source) && !file.endsWith(".test.tsx")) {
      utilityClusterViolations.push(file);
    }
  }

  for (const file of cvaScanFiles) {
    const source = read(file);
    if (source.includes('from "class-variance-authority"') && !file.endsWith(".test.tsx")) {
      cvaViolations.push(file);
    }
  }

  const arbitraryPxTextSizeBudget = 0;
  let arbitraryPxTextSizeCount = 0;
  for (const file of targetFiles) {
    if (file.endsWith(".test.tsx")) continue;
    const source = read(file);
    const matches = source.match(/text-\[[0-9]+px\]/g);
    if (matches) arbitraryPxTextSizeCount += matches.length;
  }
  if (arbitraryPxTextSizeCount > arbitraryPxTextSizeBudget) {
    failures.push(
      `Desktop inline arbitrary px text sizes regressed: ${arbitraryPxTextSizeCount} occurrences (budget ${arbitraryPxTextSizeBudget}). Use the typography classes in styles/typography.css (.text-body, .text-body-muted, .text-meta, .text-micro, .metric-*) instead of inline text-[XXpx] utilities.`,
    );
  }

  const arbitrarySpacingBudget = 0;
  let arbitrarySpacingCount = 0;
  for (const file of targetFiles) {
    if (file.endsWith(".test.tsx")) continue;
    const source = read(file);
    const matches = source.match(
      /\b(p|px|py|pt|pr|pb|pl|m|mx|my|mt|mr|mb|ml|gap|space-y|space-x)-\[[0-9.]+(?:px|rem|em)\]/g,
    );
    if (matches) arbitrarySpacingCount += matches.length;
  }
  if (arbitrarySpacingCount > arbitrarySpacingBudget) {
    failures.push(
      `Desktop inline arbitrary px spacing regressed: ${arbitrarySpacingCount} occurrences (budget ${arbitrarySpacingBudget}). Use semantic layout classes backed by the --space-* scale instead of arbitrary [Npx] values.`,
    );
  }

  for (const file of styleApiFiles) {
    const source = read(file);
    for (const classMarker of LEGACY_SURFACE_CLASS_MARKERS) {
      if (source.includes(classMarker)) {
        surfaceViolations.push(`${file} still uses legacy ${classMarker}`);
      }
    }
  }

  if (surfaceViolations.length > 0) {
    failures.push(
      `Desktop card, panel, tile, and list-row surfaces must use the shared component style API instead of legacy aliases or duplicate utility-style class strings: ${surfaceViolations.join(", ")}`,
    );
  }

  if (inlineStyleViolations.length > 0) {
    failures.push(
      `Desktop DOM styling must use shared classes/components instead of inline style props: ${inlineStyleViolations.join(", ")}`,
    );
  }
  if (rawButtonViolations.length > 0) {
    failures.push(
      `Desktop clickable actions must use the shared Button component instead of raw <button> elements: ${rawButtonViolations.join(", ")}`,
    );
  }
  if (longClassViolations.length > 0) {
    failures.push(
      `Desktop className strings over 100 characters must be moved into shared component classes: ${longClassViolations.join(", ")}`,
    );
  }
  if (utilityClusterViolations.length > 0) {
    failures.push(
      `Desktop className strings with six or more utility-shaped classes must use shared component classes: ${utilityClusterViolations.join(", ")}`,
    );
  }
  if (cvaViolations.length > 0) {
    failures.push(
      `Desktop must not introduce class-variance-authority. Use plain CSS classes from styles/ instead. Violating files: ${cvaViolations.join(", ")}`,
    );
  }

  const cardsCss = read("apps/desktop/src/styles/cards.css");
  const interactiveCss = read("apps/desktop/src/styles/interactive.css");
  const dataCss = read("apps/desktop/src/styles/data.css");
  const buttonsCss = read("apps/desktop/src/styles/buttons.css");
  const guide = read("apps/desktop/src/styles/COMPONENT_GUIDE.md");
  for (const requiredClass of [
    ".card",
    ".card--muted",
    ".card--compact",
    ".card--spacious",
    ".card--interactive",
    ".panel",
    ".panel--flush",
    ".panel__header",
    ".tile",
    ".tile__label",
    ".action-card",
    ".metric-card",
    ".project-card",
    ".integration-card",
    ".alert-row",
  ]) {
    if (!cardsCss.includes(requiredClass)) {
      failures.push(`Desktop shared style API must define ${requiredClass} in styles/cards.css.`);
    }
  }
  for (const requiredClass of [".field-shell", ".field-shell__control", ".field-control"]) {
    if (!interactiveCss.includes(requiredClass)) {
      failures.push(
        `Desktop shared style API must define ${requiredClass} in styles/interactive.css.`,
      );
    }
  }
  for (const requiredClass of [
    ".eyebrow",
    ".eyebrow--alt",
    ".disclosure-summary",
    ".disclosure-chevron",
    ".code-text-block",
    ".row-action-link",
  ]) {
    if (!dataCss.includes(requiredClass)) {
      failures.push(`Desktop shared style API must define ${requiredClass} in styles/data.css.`);
    }
  }
  for (const requiredClass of [
    ".btn",
    ".btn--default",
    ".btn--destructive",
    ".btn--outline",
    ".btn--secondary",
    ".btn--ghost",
    ".btn--link",
    ".btn--accent",
    ".btn--sm",
    ".btn--lg",
    ".btn--icon",
  ]) {
    if (!buttonsCss.includes(requiredClass)) {
      failures.push(
        `Desktop button visual spec must define ${requiredClass} in styles/buttons.css.`,
      );
    }
  }
  if (!guide.includes('className="card card--interactive card--muted action-card is-current"')) {
    failures.push(
      "Desktop component style guide must document base + modifier + role + state composition.",
    );
  }

  return failures;
}
