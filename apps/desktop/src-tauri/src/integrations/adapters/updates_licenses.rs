//! License work items for direct production npm dependencies.
//!
//! Only unlicensed packages and single-license GPL or AGPL declarations are
//! classified; all other expressions remain unjudged.

use crate::checks::Severity;
use crate::core::correlation::signal_mapping::resolve_check_id;
use crate::db::work_items::{WorkItemInput, WorkItemMetadata};
use crate::updates::types::PackageLicense;

/// How many packages each description names before deferring to the details.
const DESCRIPTION_PREVIEW_LIMIT: usize = 10;

/// How a declared license is treated by the minimal policy.
#[derive(Debug, PartialEq, Eq)]
enum LicensePosture {
    /// The whole expression is a single GPL/AGPL identifier.
    StrongCopyleft,
    /// No license declared, or npm's explicit `UNLICENSED` marker.
    NoLicense,
    /// Anything else: permissive, weak copyleft, dual-licensed, custom.
    Unjudged,
}

fn classify(license: Option<&str>) -> LicensePosture {
    let Some(raw) = license else {
        return LicensePosture::NoLicense;
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("UNLICENSED") {
        return LicensePosture::NoLicense;
    }

    // Only judge single-identifier expressions. An `OR` offers a choice of
    // license, so nothing is imposed; compound `AND` expressions are rare
    // and ambiguous enough that a wrong flag costs more than a miss.
    let upper = trimmed.to_uppercase();
    if upper.contains(" OR ") || upper.contains(" AND ") {
        return LicensePosture::Unjudged;
    }
    let identifier = upper.trim_start_matches('(').trim_end_matches(')').trim();
    if identifier.starts_with("GPL") || identifier.starts_with("AGPL") {
        return LicensePosture::StrongCopyleft;
    }
    LicensePosture::Unjudged
}

/// The aggregated copyleft and no-license work items for one project's npm
/// licenses; empty when nothing is flagged. `licenses` is expected
/// pre-sorted by name (the npm client sorts) so the copy is stable.
pub(crate) fn build_license_work_items(
    project_id: i64,
    env_url: &str,
    licenses: &[PackageLicense],
    observed_at: i64,
) -> Vec<WorkItemInput> {
    let production: Vec<&PackageLicense> = licenses.iter().filter(|entry| !entry.is_dev).collect();

    let copyleft: Vec<&PackageLicense> = production
        .iter()
        .filter(|entry| classify(entry.license.as_deref()) == LicensePosture::StrongCopyleft)
        .copied()
        .collect();
    let unlicensed: Vec<&PackageLicense> = production
        .iter()
        .filter(|entry| classify(entry.license.as_deref()) == LicensePosture::NoLicense)
        .copied()
        .collect();

    let mut items = Vec::new();
    if !copyleft.is_empty() {
        items.push(copyleft_work_item(
            project_id,
            env_url,
            &copyleft,
            observed_at,
        ));
    }
    if !unlicensed.is_empty() {
        items.push(no_license_work_item(
            project_id,
            env_url,
            &unlicensed,
            observed_at,
        ));
    }
    items
}

fn copyleft_work_item(
    project_id: i64,
    env_url: &str,
    packages: &[&PackageLicense],
    observed_at: i64,
) -> WorkItemInput {
    let title = format!(
        "{} production {} a strong copyleft license (GPL or AGPL)",
        packages.len(),
        if packages.len() == 1 {
            "dependency uses"
        } else {
            "dependencies use"
        }
    );
    let listing = preview_listing(packages, |entry| {
        entry.license.as_deref().unwrap_or("unknown").to_string()
    });

    WorkItemInput {
        project_id,
        env_url: env_url.to_string(),
        source: "updates".to_string(),
        signal_id: "updates:license-copyleft:npm".to_string(),
        check_id: resolve_check_id("updates", "license-copyleft"),
        category: "dependencies".to_string(),
        severity: Severity::Low,
        title,
        description: format!(
            "These packages declare GPL-family licenses: {}. Copyleft licenses can \
             obligate you to publish your own source code depending on how you use and \
             distribute the package. Whether that applies to this project is a decision \
             worth making deliberately rather than by accident.",
            listing
        ),
        detail_json: license_detail_json(packages),
        scan_ref: None,
        page_url: None,
        fix_prompt: None,
        manual_fix: Some(
            "Confirm each license is compatible with how you ship this project. If it is \
             not, swap the package for an alternative under a permissive license; if it \
             is, keep a note of the decision so it does not resurface as a question later."
                .to_string(),
        ),
        why_it_matters: Some(
            "License obligations attach when you distribute or serve the code, and \
             finding a copyleft dependency late can force a rewrite at the worst time."
                .to_string(),
        ),
        observed_at,
        metadata: WorkItemMetadata::default(),
    }
}

fn no_license_work_item(
    project_id: i64,
    env_url: &str,
    packages: &[&PackageLicense],
    observed_at: i64,
) -> WorkItemInput {
    let title = format!(
        "{} production {} no license",
        packages.len(),
        if packages.len() == 1 {
            "dependency declares"
        } else {
            "dependencies declare"
        }
    );
    let listing = preview_listing(packages, |entry| match entry.license.as_deref() {
        Some(marker) => marker.to_string(),
        None => "no license field".to_string(),
    });

    WorkItemInput {
        project_id,
        env_url: env_url.to_string(),
        source: "updates".to_string(),
        signal_id: "updates:license-missing:npm".to_string(),
        check_id: resolve_check_id("updates", "license-missing"),
        category: "dependencies".to_string(),
        severity: Severity::Low,
        title,
        description: format!(
            "These packages declare no license on the npm registry: {}. Without a \
             license, copyright law reserves all rights to the author; npm's UNLICENSED \
             marker states that explicitly.",
            listing
        ),
        detail_json: license_detail_json(packages),
        scan_ref: None,
        page_url: None,
        fix_prompt: None,
        manual_fix: Some(
            "Check each package's repository for a license file the registry record is \
             missing. If there genuinely is none, ask the author to add one or replace \
             the package with a licensed alternative."
                .to_string(),
        ),
        why_it_matters: Some(
            "You have no automatic right to use or redistribute unlicensed code, which \
             matters the moment this project is shipped, sold, or audited."
                .to_string(),
        ),
        observed_at,
        metadata: WorkItemMetadata::default(),
    }
}

fn preview_listing(
    packages: &[&PackageLicense],
    label: impl Fn(&PackageLicense) -> String,
) -> String {
    let preview: Vec<String> = packages
        .iter()
        .take(DESCRIPTION_PREVIEW_LIMIT)
        .map(|entry| format!("{} {} ({})", entry.name, entry.version, label(entry)))
        .collect();
    let overflow = packages.len().saturating_sub(DESCRIPTION_PREVIEW_LIMIT);
    if overflow > 0 {
        format!(
            "{} and {} more (full list in the issue details)",
            preview.join(", "),
            overflow
        )
    } else {
        preview.join(", ")
    }
}

fn license_detail_json(packages: &[&PackageLicense]) -> Option<String> {
    serde_json::to_string(&serde_json::json!({
        "ecosystem": "npm",
        "packages": packages,
    }))
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, license: Option<&str>, is_dev: bool) -> PackageLicense {
        PackageLicense {
            name: name.into(),
            version: "1.0.0".into(),
            license: license.map(str::to_string),
            is_dev,
        }
    }

    #[test]
    fn classify_flags_only_whole_gpl_family_identifiers() {
        for copyleft in ["GPL-3.0-only", "GPL-2.0", "AGPL-3.0-or-later", "gpl-3.0"] {
            assert_eq!(
                classify(Some(copyleft)),
                LicensePosture::StrongCopyleft,
                "{copyleft}"
            );
        }
        for unjudged in [
            "MIT",
            "Apache-2.0",
            "LGPL-3.0-only",          // weak copyleft: not flagged
            "(MIT OR GPL-2.0)",       // dual licensed: a choice exists
            "MIT AND GPL-2.0",        // compound: too ambiguous to judge
            "SEE LICENSE IN LICENSE", // custom license present
        ] {
            assert_eq!(
                classify(Some(unjudged)),
                LicensePosture::Unjudged,
                "{unjudged}"
            );
        }
    }

    #[test]
    fn classify_treats_missing_empty_and_unlicensed_as_no_license() {
        assert_eq!(classify(None), LicensePosture::NoLicense);
        assert_eq!(classify(Some("  ")), LicensePosture::NoLicense);
        assert_eq!(classify(Some("UNLICENSED")), LicensePosture::NoLicense);
        assert_eq!(classify(Some("unlicensed")), LicensePosture::NoLicense);
    }

    #[test]
    fn builds_one_item_per_flagged_posture() {
        let licenses = [
            entry("gpl-lib", Some("GPL-3.0-only"), false),
            entry("mystery-lib", None, false),
            entry("fine-lib", Some("MIT"), false),
        ];
        let items = build_license_work_items(7, "https://example.com", &licenses, 1_000);
        assert_eq!(items.len(), 2);

        let copyleft = &items[0];
        assert_eq!(copyleft.signal_id, "updates:license-copyleft:npm");
        assert_eq!(copyleft.check_id, "dependencies.license-copyleft");
        assert_eq!(copyleft.severity, Severity::Low);
        assert_eq!(
            copyleft.title,
            "1 production dependency uses a strong copyleft license (GPL or AGPL)"
        );
        assert!(copyleft
            .description
            .contains("gpl-lib 1.0.0 (GPL-3.0-only)"));

        let missing = &items[1];
        assert_eq!(missing.signal_id, "updates:license-missing:npm");
        assert_eq!(missing.check_id, "dependencies.license-missing");
        assert_eq!(missing.title, "1 production dependency declares no license");
        assert!(missing
            .description
            .contains("mystery-lib 1.0.0 (no license field)"));
    }

    #[test]
    fn dev_dependencies_are_never_flagged() {
        let licenses = [
            entry("gpl-cli", Some("GPL-3.0-only"), true),
            entry("unlicensed-tool", None, true),
        ];
        assert!(build_license_work_items(7, "https://example.com", &licenses, 1_000).is_empty());
    }

    #[test]
    fn all_permissive_licenses_produce_no_items() {
        let licenses = [
            entry("react", Some("MIT"), false),
            entry("axios", Some("Apache-2.0"), false),
        ];
        assert!(build_license_work_items(7, "https://example.com", &licenses, 1_000).is_empty());
    }

    #[test]
    fn description_previews_ten_packages_and_counts_the_rest() {
        let licenses: Vec<PackageLicense> = (0..13)
            .map(|i| entry(&format!("gpl-{i:02}"), Some("GPL-2.0"), false))
            .collect();
        let items = build_license_work_items(7, "https://example.com", &licenses, 1_000);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].title,
            "13 production dependencies use a strong copyleft license (GPL or AGPL)"
        );
        assert!(items[0].description.contains("gpl-09"));
        assert!(!items[0].description.contains("gpl-10"));
        assert!(items[0]
            .description
            .contains("and 3 more (full list in the issue details)"));
        assert!(items[0]
            .detail_json
            .as_deref()
            .is_some_and(|json| json.contains("gpl-12")));
    }

    #[test]
    fn unlicensed_marker_is_shown_verbatim_in_description() {
        let licenses = [entry("private-pkg", Some("UNLICENSED"), false)];
        let items = build_license_work_items(7, "https://example.com", &licenses, 1_000);
        assert_eq!(items.len(), 1);
        assert!(items[0]
            .description
            .contains("private-pkg 1.0.0 (UNLICENSED)"));
    }
}
