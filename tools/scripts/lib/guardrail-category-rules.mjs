export function desktopCategoryLabelFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const allowedCategoryLabelFiles = new Set([
    "apps/desktop/src/lib/tokens.ts",
    "apps/desktop/src/components/scan/code-scan-result-model.ts",
  ]);
  const duplicateCategoryLabelFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/") || !/\.(ts|tsx)$/.test(file)) return false;
    if (file.includes(".test.")) return false;
    if (allowedCategoryLabelFiles.has(file)) return false;
    const source = read(file);
    return (
      /\b(?:export\s+)?const\s+CATEGORY_LABELS\b/.test(source) ||
      source.includes("GUARDRAIL_CATEGORY_LABELS")
    );
  });

  const duplicateWebCategoryOrderFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/") || !/\.(ts|tsx)$/.test(file)) return false;
    if (file.includes(".test.") || allowedCategoryLabelFiles.has(file)) return false;
    return /const\s+\w*(CATEGORY_ORDER|FILTER_ORDER)\w*\s*(?::[^=]+)?=\s*\[[\s\S]{0,220}security[\s\S]{0,220}performance[\s\S]{0,220}seo[\s\S]{0,220}accessibility/.test(
      read(file),
    );
  });

  const duplicateDomainStyleFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/") || !/\.(ts|tsx)$/.test(file)) return false;
    if (file.includes(".test.")) return false;
    if (file === "apps/desktop/src/components/scan/code-scan-result-model.ts") return false;
    return /\b(?:export\s+)?const\s+DOMAIN_STYLES\b/.test(read(file));
  });

  const scanOverlayStagesSource = read("apps/desktop/src/components/scan/scan-overlay-stages.ts");
  const tokensSource = read("apps/desktop/src/lib/tokens.ts");
  const actionLanguageSource = read("apps/desktop/src/lib/action-language.ts");
  const eventsModelSource = read("apps/desktop/src/components/events/events-page-model.ts");

  check(
    tokensSource.includes('import { CATEGORY_META } from "./category-meta"') &&
      !tokensSource.includes('compliance: "Compliance"') &&
      // Either accessor is fine: both live in lib/tokens.ts and derive from
      // CATEGORY_META. Dense surfaces take the short name; the rule is that
      // the label comes from the shared module, never a literal or local map.
      /CATEGORY_(?:SHORT_)?LABELS\.compliance/.test(actionLanguageSource) &&
      eventsModelSource.includes("CATEGORY_LABELS.compliance") &&
      /label: CATEGORY_(?:SHORT_)?LABELS\.compliance/.test(scanOverlayStagesSource) &&
      // Prevent local style maps from shadowing CODE_SCAN_DOMAIN_META tokens.
      duplicateCategoryLabelFiles.length === 0 &&
      duplicateWebCategoryOrderFiles.length === 0 &&
      duplicateDomainStyleFiles.length === 0,
    `Desktop category labels/domain styles must use lib/tokens.ts or scan/code-scan-result-model.ts instead of local maps: ${[...duplicateCategoryLabelFiles, ...duplicateWebCategoryOrderFiles, ...duplicateDomainStyleFiles].join(", ")}`,
  );

  return failures;
}
