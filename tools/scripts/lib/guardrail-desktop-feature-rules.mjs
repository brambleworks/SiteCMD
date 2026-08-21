export function licenseFeatureSourceFailures(read) {
  const hookSource = read("apps/desktop/src/hooks/useTier.tsx");
  const configSource = read("apps/desktop/src-tauri/src/licensing/config.rs");
  const commandSource = read("apps/desktop/src-tauri/src/licensing/commands/mod.rs");

  if (
    configSource.includes("pub enum Feature") ||
    configSource.includes("pub fn has_feature") ||
    commandSource.includes("features_for_tier") ||
    commandSource.includes("features: Vec<Feature>") ||
    hookSource.includes("hasFeature") ||
    hookSource.includes("licenseInfo.features") ||
    hookSource.includes("FEATURE_TIERS")
  ) {
    return [
      "Client-side feature gating stays deleted: no Feature enum or has_feature in licensing/config.rs, no feature list on LicenseInfo, no hasFeature in useTier - the paid boundary is the connected service, enforced server-side.",
    ];
  }

  return [];
}
