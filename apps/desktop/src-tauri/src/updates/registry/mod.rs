//! Package registry orchestrator.
//!
//! Groups installed packages by ecosystem, dispatches concurrent registry lookups
//! and OSV vulnerability checks, then merges results into a unified update list.

pub mod concurrency;
pub mod crates_io;
pub mod drupal_api;
pub mod go_proxy;
pub mod npm_packument;
pub mod npm_registry;
pub mod osv;
pub mod packagist;
pub mod pypi;
pub mod rubygems;
pub mod wordpress_api;

use super::types::{
    Ecosystem, InstallScriptPackage, InstalledPackage, PackageLicense, PackageUpdate,
};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

type PackageKey = (String, String);

/// Updates plus npm lifecycle-script and license posture from one sweep.
#[derive(Default, Clone)]
pub struct RegistryScan {
    pub updates: Vec<PackageUpdate>,
    pub install_script_packages: Vec<InstallScriptPackage>,
    pub licenses: Vec<PackageLicense>,
    /// Whether the dependency census was incomplete.
    /// Callers must not resolve absent findings from a partial sweep.
    pub partial: bool,
}

/// Age threshold for informational package-maintenance findings.
pub(crate) const STALE_AFTER_DAYS: i64 = 3 * 365;

/// Return whether a parseable publish timestamp exceeds the stale threshold.
pub(crate) fn is_stale_at(last_published: Option<&str>, now: DateTime<Utc>) -> bool {
    let Some(timestamp) = last_published else {
        return false;
    };
    let Ok(published) = DateTime::parse_from_rfc3339(timestamp) else {
        return false;
    };
    published_is_stale(published.with_timezone(&Utc), now)
}

/// Strict stale threshold for registries with pre-parsed timestamps.
pub(crate) fn published_is_stale(published: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(published) > chrono::Duration::days(STALE_AFTER_DAYS)
}

/// Treat only 404 as an observed package absence.
/// Rate limits, outages, and other errors leave the package unobserved so
/// existing findings cannot resolve from incomplete evidence.
pub(crate) fn status_is_observed_absence(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::NOT_FOUND
}

/// Merge findings and propagate partial-observation state across ecosystems.
fn merge_ecosystem_scan(sweep: &mut RegistryScan, ecosystem_scan: RegistryScan) {
    sweep.updates.extend(ecosystem_scan.updates);
    sweep
        .install_script_packages
        .extend(ecosystem_scan.install_script_packages);
    sweep.licenses.extend(ecosystem_scan.licenses);
    sweep.partial |= ecosystem_scan.partial;
}

/// Query ecosystem registries and OSV while retaining npm install-script posture.
pub async fn check_for_updates(packages: &[InstalledPackage]) -> RegistryScan {
    let mut by_ecosystem: HashMap<Ecosystem, Vec<&InstalledPackage>> = HashMap::new();
    for pkg in packages {
        by_ecosystem
            .entry(pkg.ecosystem.clone())
            .or_default()
            .push(pkg);
    }

    let packages_for_osv: Vec<InstalledPackage> = packages.to_vec();
    let osv_handle =
        tokio::spawn(async move { osv::check_vulnerabilities(&packages_for_osv).await });

    let mut sweep = RegistryScan::default();
    let mut handles = Vec::new();

    for (ecosystem, pkgs) in by_ecosystem {
        let owned: Vec<InstalledPackage> = pkgs.into_iter().cloned().collect();
        handles.push(tokio::spawn(async move {
            query_ecosystem(&ecosystem, &owned).await
        }));
    }

    for handle in handles {
        match handle.await {
            Ok(ecosystem_scan) => merge_ecosystem_scan(&mut sweep, ecosystem_scan),
            Err(e) => {
                tracing::warn!("updates: registry query failed: {}", e);
                // That ecosystem's census was not observed; its findings must
                // not read as resolved.
                sweep.partial = true;
            }
        }
    }

    // Merge OSV vulnerability data. A failed batch or a dead OSV task means
    // the vulnerability census was not observed: the sweep is partial so
    // vulnerability items survive an OSV outage instead of false-resolving.
    let vulns = match osv_handle.await {
        Ok(osv_scan) => {
            sweep.partial |= osv_scan.partial;
            osv_scan.vulns
        }
        Err(e) => {
            tracing::warn!("updates: OSV query failed: {}", e);
            sweep.partial = true;
            Vec::new()
        }
    };

    if !vulns.is_empty() {
        tracing::info!("updates: OSV found {} vulnerable packages", vulns.len());

        let remediation_candidates = remediation_candidates(&sweep.updates, &vulns);
        let verified_remediations = if remediation_candidates.is_empty() {
            HashSet::new()
        } else {
            let candidate_scan = osv::check_vulnerabilities(&remediation_candidates).await;
            verified_remediation_keys(
                &remediation_candidates,
                &candidate_scan.vulns,
                candidate_scan.partial,
            )
        };

        // Build a lookup: (ecosystem, name) -> vulnerability info
        let vuln_map: HashMap<PackageKey, &osv::VulnerabilityInfo> = vulns
            .iter()
            .map(|v| ((v.ecosystem.label().to_string(), v.package_name.clone()), v))
            .collect();

        // Mark existing updates as security-related
        for update in &mut sweep.updates {
            let key = (update.ecosystem.label().to_string(), update.name.clone());
            if let Some(vuln) = vuln_map.get(&key) {
                update.is_security = true;
                update.advisory_severity = Some(vuln.severity.as_str().to_string());
                update.advisory_url = vuln.advisory_url.clone();
                update.advisory_fixed_version = verified_remediations
                    .contains(&key)
                    .then(|| update.latest_version.clone());
            }
        }

        // Keep vulnerabilities even when no registry update exists.
        let update_names: std::collections::HashSet<(String, String)> = sweep
            .updates
            .iter()
            .map(|u| (u.ecosystem.label().to_string(), u.name.clone()))
            .collect();

        for vuln in &vulns {
            let key = (
                vuln.ecosystem.label().to_string(),
                vuln.package_name.clone(),
            );
            if !update_names.contains(&key) {
                sweep.updates.push(PackageUpdate {
                    name: vuln.package_name.clone(),
                    current_version: vuln.current_version.clone(),
                    latest_version: vuln.current_version.clone(),
                    ecosystem: vuln.ecosystem.clone(),
                    update_type: super::types::UpdateType::Unknown,
                    is_security: true,
                    advisory_severity: Some(vuln.severity.as_str().to_string()),
                    advisory_url: vuln.advisory_url.clone(),
                    advisory_fixed_version: None,
                    source: vuln.source.clone(),
                    is_dev: vuln.is_dev,
                    workspace_members: vuln.workspace_members.clone(),
                    ..Default::default()
                });
            }
        }
    }

    // Sort: security first, then by ecosystem, then by name
    sweep.updates.sort_by(|a, b| {
        b.is_security
            .cmp(&a.is_security)
            .then(format!("{:?}", a.ecosystem).cmp(&format!("{:?}", b.ecosystem)))
            .then(a.name.cmp(&b.name))
    });

    sweep
}

fn package_key(ecosystem: &Ecosystem, name: &str) -> PackageKey {
    (ecosystem.label().to_string(), name.to_string())
}

fn remediation_candidates(
    updates: &[PackageUpdate],
    vulnerabilities: &[osv::VulnerabilityInfo],
) -> Vec<InstalledPackage> {
    let vulnerable: HashSet<PackageKey> = vulnerabilities
        .iter()
        .map(|vulnerability| package_key(&vulnerability.ecosystem, &vulnerability.package_name))
        .collect();

    updates
        .iter()
        .filter(|update| vulnerable.contains(&package_key(&update.ecosystem, &update.name)))
        .map(|update| InstalledPackage {
            name: update.name.clone(),
            version: update.latest_version.clone(),
            ecosystem: update.ecosystem.clone(),
            source: update.source.clone(),
            is_dev: update.is_dev,
            workspace_members: update.workspace_members.clone(),
        })
        .collect()
}

fn verified_remediation_keys(
    candidates: &[InstalledPackage],
    candidate_vulnerabilities: &[osv::VulnerabilityInfo],
    partial: bool,
) -> HashSet<PackageKey> {
    if partial {
        return HashSet::new();
    }
    let still_vulnerable: HashSet<PackageKey> = candidate_vulnerabilities
        .iter()
        .map(|vulnerability| package_key(&vulnerability.ecosystem, &vulnerability.package_name))
        .collect();
    candidates
        .iter()
        .map(|candidate| package_key(&candidate.ecosystem, &candidate.name))
        .filter(|key| !still_vulnerable.contains(key))
        .collect()
}

/// One ecosystem's contribution to the sweep. Only npm carries the
/// install-script and license channels; the update-only ecosystems fill
/// `updates` plus the ecosystem's own partial flag.
async fn query_ecosystem(ecosystem: &Ecosystem, packages: &[InstalledPackage]) -> RegistryScan {
    let updates_only = |(updates, partial): (Vec<PackageUpdate>, bool)| RegistryScan {
        updates,
        partial,
        ..Default::default()
    };
    match ecosystem {
        Ecosystem::Npm => {
            let scan = npm_registry::check_updates(packages).await;
            RegistryScan {
                updates: scan.updates,
                install_script_packages: scan.install_script_packages,
                licenses: scan.licenses,
                partial: scan.partial,
            }
        }
        Ecosystem::Composer => updates_only(packagist::check_updates(packages).await),
        Ecosystem::Python => updates_only(pypi::check_updates(packages).await),
        Ecosystem::Ruby => updates_only(rubygems::check_updates(packages).await),
        Ecosystem::Go => updates_only(go_proxy::check_updates(packages).await),
        Ecosystem::Rust => updates_only(crates_io::check_updates(packages).await),
        Ecosystem::WordPress => updates_only(wordpress_api::check_updates(packages).await),
        Ecosystem::Drupal => updates_only(drupal_api::check_updates(packages).await),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Severity;

    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .expect("valid fixture timestamp")
            .with_timezone(&Utc)
    }

    fn vulnerability(name: &str) -> osv::VulnerabilityInfo {
        osv::VulnerabilityInfo {
            package_name: name.to_string(),
            ecosystem: Ecosystem::Npm,
            current_version: "1.0.0".to_string(),
            source: "package-lock.json".to_string(),
            is_dev: true,
            workspace_members: vec!["apps/web".to_string()],
            advisory_id: format!("GHSA-{name}"),
            severity: Severity::High,
            summary: "Test advisory".to_string(),
            advisory_url: None,
        }
    }

    #[test]
    fn stale_boundary_is_three_years_from_injected_now() {
        let now = fixed_now();
        // One day PAST the 3-year threshold: stale.
        let over = (now - chrono::Duration::days(STALE_AFTER_DAYS + 1)).to_rfc3339();
        assert!(is_stale_at(Some(&over), now));
        // One day WITHIN the threshold: not stale.
        let under = (now - chrono::Duration::days(STALE_AFTER_DAYS - 1)).to_rfc3339();
        assert!(!is_stale_at(Some(&under), now));
        // Exactly at the threshold: not stale ("more than 3 years" is strict).
        let exact = (now - chrono::Duration::days(STALE_AFTER_DAYS)).to_rfc3339();
        assert!(!is_stale_at(Some(&exact), now));
    }

    #[test]
    fn missing_or_unparseable_publish_time_is_not_stale() {
        assert!(!is_stale_at(None, fixed_now()));
        assert!(!is_stale_at(Some("not a timestamp"), fixed_now()));
    }

    #[tokio::test]
    async fn empty_sweep_is_complete_not_partial() {
        let scan = check_for_updates(&[]).await;
        assert!(scan.updates.is_empty());
        assert!(!scan.partial);
    }

    #[test]
    fn published_is_stale_uses_same_strict_boundary() {
        let now = fixed_now();
        assert!(published_is_stale(
            now - chrono::Duration::days(STALE_AFTER_DAYS + 1),
            now
        ));
        assert!(!published_is_stale(
            now - chrono::Duration::days(STALE_AFTER_DAYS),
            now
        ));
    }

    #[test]
    fn only_404_counts_as_an_observed_absence() {
        use reqwest::StatusCode;
        // 404 = the registry authoritatively does not know the package
        // (private/unpublished dependency): a real absence, never partial.
        assert!(status_is_observed_absence(StatusCode::NOT_FOUND));
        // Outage-class and surprise statuses leave the package unobserved and
        // must count toward a partial sweep.
        for status in [
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::FORBIDDEN,
        ] {
            assert!(
                !status_is_observed_absence(status),
                "{status} must count toward a partial sweep"
            );
        }
    }

    #[test]
    fn merge_ecosystem_scan_propagates_partial_and_keeps_findings() {
        let mut sweep = RegistryScan::default();

        merge_ecosystem_scan(&mut sweep, RegistryScan::default());
        assert!(
            !sweep.partial,
            "complete ecosystem scans keep the sweep complete"
        );

        let partial_with_finding = RegistryScan {
            updates: vec![PackageUpdate {
                name: "left-pad".into(),
                ..Default::default()
            }],
            partial: true,
            ..Default::default()
        };
        merge_ecosystem_scan(&mut sweep, partial_with_finding);
        assert!(
            sweep.partial,
            "one partial ecosystem must make the whole sweep partial"
        );
        assert_eq!(
            sweep.updates.len(),
            1,
            "observed findings still accumulate on a partial merge"
        );

        // Partial is sticky: a later complete ecosystem must not clear it.
        merge_ecosystem_scan(&mut sweep, RegistryScan::default());
        assert!(sweep.partial);
    }

    #[test]
    fn remediation_candidates_use_registry_versions_and_package_metadata() {
        let updates = vec![PackageUpdate {
            name: "lodash".to_string(),
            latest_version: "4.17.21".to_string(),
            ecosystem: Ecosystem::Npm,
            source: "pnpm-lock.yaml".to_string(),
            is_dev: true,
            workspace_members: vec!["apps/web".to_string()],
            ..Default::default()
        }];

        let candidates = remediation_candidates(&updates, &[vulnerability("lodash")]);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].version, "4.17.21");
        assert_eq!(candidates[0].source, "pnpm-lock.yaml");
        assert!(candidates[0].is_dev);
        assert_eq!(candidates[0].workspace_members, ["apps/web"]);
    }

    #[test]
    fn remediation_requires_a_complete_clean_candidate_scan() {
        let candidate = InstalledPackage {
            name: "lodash".to_string(),
            version: "4.17.21".to_string(),
            ecosystem: Ecosystem::Npm,
            ..Default::default()
        };
        let candidates = [candidate];

        assert!(verified_remediation_keys(&candidates, &[], false)
            .contains(&("npm".to_string(), "lodash".to_string())));
        assert!(
            verified_remediation_keys(&candidates, &[vulnerability("lodash")], false).is_empty()
        );
        assert!(verified_remediation_keys(&candidates, &[], true).is_empty());
    }
}
