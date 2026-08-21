//! Lockfile, registry, TLS, and GitHub CI update signals.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::checks::Severity;
use crate::core::correlation::signal_mapping::resolve_check_id;
use crate::db::alerts::AlertInput;
use crate::db::work_items::{WorkItemInput, WorkItemMetadata};
use crate::db::Database;
use crate::integrations::adapters::{AdapterError, IntegrationAdapter, PollContext, PollOutput};
use crate::updates::ci::CiFailure;
use crate::updates::types::{PackageUpdate, UpdateType};

/// Dependency prefixes marked unobservable after a partial package census.
/// TLS and CI signals must continue resolving independently.
const DEPENDENCY_SIGNAL_PREFIXES: [&str; 6] = [
    "updates:vulnerability:",
    "updates:deprecated:",
    "updates:outdated-major:",
    "updates:install-scripts:",
    "updates:license-copyleft:",
    "updates:license-missing:",
];

/// Prefix of the SSL-expiry family, reported unobservable when the TLS probe
/// fails.
const SSL_SIGNAL_PREFIX: &str = "updates:ssl-expiring:";

/// Prefix of the CI-failure family, reported unobservable when the GitHub
/// fetch fails.
const CI_SIGNAL_PREFIX: &str = "updates:ci-failure:";

pub struct UpdatesAdapter {
    pub(crate) db: Arc<Database>,
    /// Project-scoped dependency results shared across environment polls.
    dep_scan_cache: super::updates_dependency_scan::DependencyScanCache,
}

impl UpdatesAdapter {
    #[tracing::instrument(skip(db))]
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            dep_scan_cache: super::updates_dependency_scan::DependencyScanCache::default(),
        }
    }
}

#[async_trait]
impl IntegrationAdapter for UpdatesAdapter {
    fn source(&self) -> &'static str {
        "updates"
    }

    fn cadence(&self) -> Duration {
        // allow-inline-duration: per-adapter polling cadence.
        Duration::from_secs(3600) // 1 hour
    }

    /// Opts in explicitly (trait default is fail closed): reads local lockfiles + public registries by name, disclosed egress, no credentials.
    fn is_configured(&self, _credentials: &crate::integrations::adapters::Credentials) -> bool {
        true
    }

    async fn poll(&self, ctx: &PollContext) -> Result<PollOutput, AdapterError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut work_items: Vec<WorkItemInput> = Vec::new();
        let mut alerts: Vec<AlertInput> = Vec::new();

        // Track partial observations per signal family to prevent false resolution.
        let mut unobserved_signal_prefixes: Vec<String> = Vec::new();
        let (scan, dependency_partial) = self
            .dep_scan_cache
            .scan_or_partial(&self.db, self.cadence(), ctx.project_id)
            .await;
        if dependency_partial {
            unobserved_signal_prefixes.extend(
                DEPENDENCY_SIGNAL_PREFIXES
                    .iter()
                    .map(|prefix| prefix.to_string()),
            );
        }

        // Aggregated posture items (install scripts, licenses) come from the
        // full scan; the per-package loop below only sees packages with
        // updates to report.
        work_items.extend(
            super::updates_install_scripts::build_install_scripts_work_item(
                ctx.project_id,
                ctx.env_url.clone(),
                &scan.install_script_packages,
                now_ms,
            ),
        );
        work_items.extend(super::updates_licenses::build_license_work_items(
            ctx.project_id,
            &ctx.env_url,
            &scan.licenses,
            now_ms,
        ));

        for pkg in &scan.updates {
            let ecosystem_label = pkg.ecosystem.label();

            if pkg.is_security {
                let severity = advisory_severity_or_default(pkg.advisory_severity.as_deref());

                let signal_id = format!("updates:vulnerability:{}:{}", ecosystem_label, pkg.name);

                let detail_json = serde_json::to_string(&serde_json::json!({
                    "package": pkg.name,
                    "ecosystem": ecosystem_label,
                    "current_version": pkg.current_version,
                    "latest_version": pkg.latest_version,
                    "advisory_fixed_version": pkg.advisory_fixed_version,
                    "advisory_severity": pkg.advisory_severity,
                    "advisory_url": pkg.advisory_url,
                    "is_dev": pkg.is_dev,
                }))
                .ok();

                work_items.push(WorkItemInput {
                    project_id: ctx.project_id,
                    env_url: ctx.env_url.clone(),
                    source: "updates".to_string(),
                    signal_id,
                    check_id: resolve_check_id("updates", "vulnerability"),
                    category: "dependencies".to_string(),
                    severity,
                    title: format!(
                        "Vulnerability in {} {} ({})",
                        pkg.name, pkg.current_version, ecosystem_label
                    ),
                    description: security_update_description(pkg),
                    detail_json,
                    scan_ref: None,
                    page_url: None,
                    fix_prompt: None,
                    manual_fix: None,
                    why_it_matters: None,
                    observed_at: now_ms,
                    metadata: WorkItemMetadata::default(),
                });

                if let Some(alert) =
                    build_security_update_alert(ctx.project_id, ctx.env_url.as_str(), pkg, now_ms)
                {
                    alerts.push(alert);
                }
            } else if pkg.is_deprecated {
                // Deprecated-package work item (no alert: important but
                // not urgent the way a critical advisory is)
                work_items.push(build_deprecated_work_item(
                    ctx.project_id,
                    ctx.env_url.clone(),
                    pkg,
                    now_ms,
                ));
            } else if pkg.update_type == UpdateType::Major {
                let signal_id = format!("updates:outdated-major:{}:{}", ecosystem_label, pkg.name);

                let detail_json = serde_json::to_string(&serde_json::json!({
                    "package": pkg.name,
                    "ecosystem": ecosystem_label,
                    "current_version": pkg.current_version,
                    "latest_version": pkg.latest_version,
                    "is_dev": pkg.is_dev,
                    "workspace_members": pkg.workspace_members,
                }))
                .ok();

                // In a workspace the same package can be declared by several
                // members, and the upgrade has to be applied in each - so the
                // description names them rather than leaving the reader to
                // grep for it.
                let where_to_apply = if pkg.workspace_members.is_empty() {
                    String::new()
                } else {
                    format!(" Declared in: {}.", pkg.workspace_members.join(", "))
                };

                work_items.push(WorkItemInput {
                    project_id: ctx.project_id,
                    env_url: ctx.env_url.clone(),
                    source: "updates".to_string(),
                    signal_id,
                    check_id: resolve_check_id("updates", "outdated-major"),
                    category: "dependencies".to_string(),
                    severity: Severity::Low,
                    title: format!(
                        "{} has a major update ({} -> {})",
                        pkg.name, pkg.current_version, pkg.latest_version
                    ),
                    description: format!(
                        "{} {} can be upgraded to {} (major version bump).{}",
                        pkg.name, pkg.current_version, pkg.latest_version, where_to_apply
                    ),
                    detail_json,
                    scan_ref: None,
                    page_url: None,
                    fix_prompt: None,
                    manual_fix: None,
                    why_it_matters: None,
                    observed_at: now_ms,
                    metadata: WorkItemMetadata::default(),
                });
            }
            // Minor / patch / unknown updates are not surfaced as work items.
        }

        match crate::updates::ssl::check_cert_expiry(&ctx.env_url).await {
            Ok(Some(cert)) if cert.days_until_expiry <= 60 => {
                let severity = match cert.days_until_expiry {
                    d if d < 7 => Severity::Critical,
                    d if d < 30 => Severity::High,
                    _ => Severity::Medium,
                };
                work_items.push(WorkItemInput {
                    project_id: ctx.project_id,
                    env_url: ctx.env_url.clone(),
                    source: "updates".to_string(),
                    signal_id: format!("updates:ssl-expiring:{}", cert.host),
                    check_id: resolve_check_id("updates", "ssl-expiring"),
                    category: "infrastructure".to_string(),
                    severity,
                    title: super::updates_adapter_ssl::ssl_expiry_title(cert.days_until_expiry),
                    description: super::updates_adapter_ssl::ssl_expiry_description(
                        &cert.host,
                        cert.days_until_expiry,
                        &cert.not_after.format("%Y-%m-%d").to_string(),
                    ),
                    detail_json: Some(serde_json::to_string(&cert).unwrap_or_default()),
                    scan_ref: None,
                    page_url: None,
                    fix_prompt: None,
                    manual_fix: None,
                    why_it_matters: None,
                    observed_at: now_ms,
                    metadata: WorkItemMetadata::default(),
                });
                if let Some(alert) = super::updates_adapter_ssl::build_ssl_expiry_alert(
                    ctx.project_id,
                    ctx.env_url.as_str(),
                    &cert,
                    now_ms,
                ) {
                    alerts.push(alert);
                }
            }
            Ok(_) => {}
            // Preserve SSL findings when this tick could not observe a certificate.
            Err(e) => {
                tracing::warn!("updates_adapter: ssl check failed: {}", e);
                unobserved_signal_prefixes.push(SSL_SIGNAL_PREFIX.to_string());
            }
        }

        // CI failure requires a scheduler-hydrated GitHub integration.
        if let Some(gh) = &ctx.credentials.github {
            let repo_spec = format!("{}/{}", gh.owner, gh.repo);
            match crate::updates::ci::latest_ci_failure(&gh.token, &repo_spec).await {
                Ok(Some(ci)) => {
                    work_items.push(WorkItemInput {
                        project_id: ctx.project_id,
                        env_url: ctx.env_url.clone(),
                        source: "updates".to_string(),
                        signal_id: format!("updates:ci-failure:{}:{}", ci.workflow_name, ci.run_id),
                        check_id: resolve_check_id("updates", "ci-failure"),
                        category: "infrastructure".to_string(),
                        severity: Severity::High,
                        title: format!("CI failing: {}", ci.workflow_name),
                        description: format!(
                            "Run {} on commit {} ({}).",
                            ci.run_id,
                            ci.commit_sha.chars().take(7).collect::<String>(),
                            ci.conclusion
                        ),
                        detail_json: Some(serde_json::to_string(&ci).unwrap_or_default()),
                        scan_ref: None,
                        page_url: None,
                        fix_prompt: None,
                        manual_fix: None,
                        why_it_matters: None,
                        observed_at: now_ms,
                        metadata: WorkItemMetadata::default(),
                    });
                    alerts.push(build_ci_failure_alert(ctx.project_id, &ci, now_ms));
                }
                Ok(None) => {}
                // Could not observe CI state this tick (GitHub 5xx / rate limit
                // / transient token failure): the ci-failure family is
                // unobserved so its items are refreshed-not-resolved.
                Err(e) => {
                    tracing::warn!("updates_adapter: ci fetch failed: {}", e);
                    unobserved_signal_prefixes.push(CI_SIGNAL_PREFIX.to_string());
                }
            }
        } else if ctx.credentials.github_unobservable {
            // Preserve CI findings when configured GitHub state was unobservable;
            // an intentionally absent GitHub integration may resolve stale findings.
            unobserved_signal_prefixes.push(CI_SIGNAL_PREFIX.to_string());
        }

        Ok(PollOutput {
            work_items,
            alerts,
            partial: false,
            unobserved_signal_prefixes,
        })
    }
}

/// Build a work item for a registry-deprecated package.
fn build_deprecated_work_item(
    project_id: i64,
    env_url: String,
    pkg: &PackageUpdate,
    observed_at: i64,
) -> WorkItemInput {
    let ecosystem_label = pkg.ecosystem.label();
    let description = match pkg.deprecation_message.as_deref() {
        Some(message) => format!("The maintainer marked {} deprecated: {}", pkg.name, message),
        None => format!(
            "The maintainer marked {} deprecated. Plan a replacement.",
            pkg.name
        ),
    };

    let detail_json = serde_json::to_string(&serde_json::json!({
        "package": pkg.name,
        "ecosystem": ecosystem_label,
        "current_version": pkg.current_version,
        "latest_version": pkg.latest_version,
        "deprecation_message": pkg.deprecation_message,
        "is_dev": pkg.is_dev,
    }))
    .ok();

    WorkItemInput {
        project_id,
        env_url,
        source: "updates".to_string(),
        signal_id: format!("updates:deprecated:{}:{}", ecosystem_label, pkg.name),
        check_id: resolve_check_id("updates", "deprecated"),
        category: "dependencies".to_string(),
        severity: Severity::Medium,
        title: format!("{} is deprecated ({})", pkg.name, ecosystem_label),
        description,
        detail_json,
        scan_ref: None,
        page_url: None,
        fix_prompt: None,
        manual_fix: None,
        why_it_matters: None,
        observed_at,
        metadata: WorkItemMetadata::default(),
    }
}

fn build_security_update_alert(
    project_id: i64,
    env_url: &str,
    pkg: &PackageUpdate,
    observed_at: i64,
) -> Option<AlertInput> {
    if !pkg.is_security || !is_alert_worthy_advisory(pkg.advisory_severity.as_deref()) {
        return None;
    }

    let advisory = advisory_severity_or_default(pkg.advisory_severity.as_deref());
    // Alerts speak the separate critical/warn alert vocabulary.
    let alert_severity = if advisory == Severity::Critical {
        "critical"
    } else {
        "warn"
    };
    let ecosystem_label = pkg.ecosystem.label();
    let advisory_key = pkg
        .advisory_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(advisory.as_str());
    let description = security_update_description(pkg);

    Some(AlertInput {
        project_id,
        // Dependency risk is project-level. Leave env empty so multi-env
        // projects do not get duplicate package alerts for the same repo.
        env_url: None,
        source: "updates".to_string(),
        alert_id: format!(
            "security-update:{}:{}:{}:{}",
            ecosystem_label, pkg.name, pkg.current_version, advisory_key
        ),
        severity: alert_severity.to_string(),
        title: format!("{} vulnerability in {}", advisory.label(), pkg.name),
        description,
        detail_json: Some(
            serde_json::json!({
                "alert_type": "security_update",
                "package": pkg.name,
                "ecosystem": ecosystem_label,
                "current_version": pkg.current_version,
                "latest_version": pkg.latest_version,
                "advisory_fixed_version": pkg.advisory_fixed_version,
                "advisory_severity": pkg.advisory_severity,
                "advisory_url": pkg.advisory_url,
                "source": pkg.source,
                "is_dev": pkg.is_dev,
                "url": env_url,
                "destination": "updates"
            })
            .to_string(),
        ),
        occurred_at: observed_at,
        observed_at,
    })
}

fn security_update_description(pkg: &PackageUpdate) -> String {
    match pkg.advisory_fixed_version.as_deref() {
        Some(version) => format!(
            "{} {} has a known security advisory. Update to {} and review the advisory scope before deploying the affected package.",
            pkg.name, pkg.current_version, version
        ),
        None => format!(
            "{} {} has a known security advisory with no verified fixed release. Review the advisory scope and apply a compensating control or replacement.",
            pkg.name, pkg.current_version
        ),
    }
}

fn build_ci_failure_alert(project_id: i64, ci: &CiFailure, observed_at: i64) -> AlertInput {
    AlertInput {
        project_id,
        env_url: None,
        source: "github".to_string(),
        alert_id: format!("ci-failure:{}:{}", ci.workflow_name, ci.run_id),
        severity: "warn".to_string(),
        title: format!("CI failed: {}", ci.workflow_name),
        description: format!(
            "GitHub Actions reported {} for run {} on commit {}. Open the run to identify the failed job before retrying or deploying.",
            ci.conclusion,
            ci.run_id,
            ci.commit_sha.chars().take(7).collect::<String>()
        ),
        detail_json: Some(
            serde_json::json!({
                "alert_type": "ci_failure",
                "workflow_name": ci.workflow_name,
                "run_id": ci.run_id,
                "conclusion": ci.conclusion,
                "html_url": ci.html_url,
                "commit_sha": ci.commit_sha,
                "completed_at": ci.completed_at,
                "destination": "deploys"
            })
            .to_string(),
        ),
        occurred_at: observed_at,
        observed_at,
    }
}

/// Normalize advisory severity at ingestion, defaulting unknown values to High.
fn advisory_severity_or_default(raw: Option<&str>) -> Severity {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.to_ascii_lowercase().parse().unwrap_or_else(|_| {
            tracing::warn!(
                advisory_severity = %value,
                "unrecognized advisory severity; defaulting to high"
            );
            Severity::High
        }),
        None => Severity::High,
    }
}

/// A missing advisory severity stays alert-worthy (assume the worst); a
/// present-but-below-High one does not.
fn is_alert_worthy_advisory(severity: Option<&str>) -> bool {
    severity.is_none_or(|value| {
        matches!(
            value.trim().to_ascii_lowercase().parse::<Severity>(),
            Ok(Severity::Critical | Severity::High)
        )
    })
}

#[cfg(test)]
#[path = "updates_adapter_tests.rs"]
mod tests;
