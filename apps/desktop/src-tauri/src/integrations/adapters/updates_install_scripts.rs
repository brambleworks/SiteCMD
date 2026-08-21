//! Aggregated npm lifecycle-script work item.
//!
//! One item per project prevents common install scripts from flooding the
//! issue list and lets source diffing resolve the whole set.

use crate::checks::Severity;
use crate::core::correlation::signal_mapping::resolve_check_id;
use crate::db::work_items::{WorkItemInput, WorkItemMetadata};
use crate::updates::types::InstallScriptPackage;

/// How many packages the description names before deferring to the details.
const DESCRIPTION_PREVIEW_LIMIT: usize = 10;

/// The one aggregated install-scripts work item, or None when no scanned
/// package declares any. `packages` is expected pre-sorted by name (the npm
/// client sorts) so the copy is stable across polls.
pub(crate) fn build_install_scripts_work_item(
    project_id: i64,
    env_url: String,
    packages: &[InstallScriptPackage],
    observed_at: i64,
) -> Option<WorkItemInput> {
    if packages.is_empty() {
        return None;
    }

    let title = if packages.len() == 1 {
        "1 direct dependency runs npm install scripts".to_string()
    } else {
        format!(
            "{} direct dependencies run npm install scripts",
            packages.len()
        )
    };

    let detail_json = serde_json::to_string(&serde_json::json!({
        "ecosystem": "npm",
        "packages": packages,
    }))
    .ok();

    Some(WorkItemInput {
        project_id,
        env_url,
        source: "updates".to_string(),
        signal_id: "updates:install-scripts:npm".to_string(),
        check_id: resolve_check_id("updates", "install-scripts"),
        category: "dependencies".to_string(),
        severity: Severity::Low,
        title,
        description: build_description(packages),
        detail_json,
        scan_ref: None,
        page_url: None,
        fix_prompt: None,
        manual_fix: Some(
            "Review each package's install script on npmjs.com and confirm you expect it \
             (native modules usually need one). Where possible install with --ignore-scripts, \
             or use a package manager that requires approving install scripts (pnpm does by \
             default), and keep a release-age quarantine so brand-new versions wait before \
             they can run anything."
                .to_string(),
        ),
        why_it_matters: Some(
            "Install scripts run with your user account on every fresh install and in CI, \
             so one compromised version of any of these packages can read tokens and modify \
             files the moment it is installed."
                .to_string(),
        ),
        observed_at,
        metadata: WorkItemMetadata::default(),
    })
}

fn build_description(packages: &[InstallScriptPackage]) -> String {
    let preview: Vec<String> = packages
        .iter()
        .take(DESCRIPTION_PREVIEW_LIMIT)
        .map(|pkg| format!("{} {} ({})", pkg.name, pkg.version, pkg.scripts.join(", ")))
        .collect();
    let overflow = packages.len().saturating_sub(DESCRIPTION_PREVIEW_LIMIT);
    let listing = if overflow > 0 {
        format!(
            "{} and {} more (full list in the issue details)",
            preview.join(", "),
            overflow
        )
    } else {
        preview.join(", ")
    };

    format!(
        "These packages execute their own commands while npm installs them: {}. \
         Install scripts are how native modules build, and also the main way a hijacked \
         package version runs code on your machine.",
        listing
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, scripts: &[&str]) -> InstallScriptPackage {
        InstallScriptPackage {
            name: name.into(),
            version: "1.0.0".into(),
            scripts: scripts.iter().map(|s| s.to_string()).collect(),
            is_dev: false,
        }
    }

    #[test]
    fn no_packages_yields_no_work_item() {
        assert!(
            build_install_scripts_work_item(7, "https://example.com".into(), &[], 1_000).is_none()
        );
    }

    #[test]
    fn aggregates_packages_into_one_low_severity_item() {
        let packages = [pkg("esbuild", &["postinstall"]), pkg("sharp", &["install"])];
        let item =
            build_install_scripts_work_item(7, "https://example.com".into(), &packages, 1_000)
                .expect("work item");

        assert_eq!(item.source, "updates");
        assert_eq!(item.signal_id, "updates:install-scripts:npm");
        assert_eq!(item.check_id, "dependencies.install-scripts");
        assert_eq!(item.category, "dependencies");
        assert_eq!(item.severity, Severity::Low);
        assert_eq!(item.title, "2 direct dependencies run npm install scripts");
        assert!(item.description.contains("esbuild 1.0.0 (postinstall)"));
        assert!(item.description.contains("sharp 1.0.0 (install)"));
        assert!(item
            .detail_json
            .as_deref()
            .is_some_and(|json| json.contains("\"esbuild\"")));
    }

    #[test]
    fn singular_title_for_one_package() {
        let packages = [pkg("bcrypt", &["install"])];
        let item =
            build_install_scripts_work_item(7, "https://example.com".into(), &packages, 1_000)
                .expect("work item");
        assert_eq!(item.title, "1 direct dependency runs npm install scripts");
    }

    #[test]
    fn description_previews_ten_packages_and_counts_the_rest() {
        let packages: Vec<InstallScriptPackage> = (0..14)
            .map(|i| pkg(&format!("pkg-{i:02}"), &["postinstall"]))
            .collect();
        let item =
            build_install_scripts_work_item(7, "https://example.com".into(), &packages, 1_000)
                .expect("work item");

        assert!(item.description.contains("pkg-09"));
        assert!(!item.description.contains("pkg-10"));
        assert!(item
            .description
            .contains("and 4 more (full list in the issue details)"));
        // The details must still carry every package.
        assert!(item
            .detail_json
            .as_deref()
            .is_some_and(|json| json.contains("pkg-13")));
    }
}
