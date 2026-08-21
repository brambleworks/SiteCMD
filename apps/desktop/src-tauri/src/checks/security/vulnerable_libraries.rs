//! OSV lookup transport for the portable vulnerable-library check.

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use crate::updates::registry::osv;
use crate::updates::types::{Ecosystem, InstalledPackage};
use sitecmd_engine::checks::security::vulnerable_libraries::{
    detect_libraries, evaluate_vulnerable_libraries, AdvisoryLookup, DetectedLibrary,
    LibraryAdvisory,
};

pub struct VulnerableLibrariesCheck;

/// Distinguish an empty OSV result from an unreachable service.
async fn osv_reachable() -> bool {
    match crate::http_client::client()
        .post("https://api.osv.dev/v1/querybatch")
        .json(&serde_json::json!({ "queries": [] }))
        .timeout(crate::constants::CHECK_PROBE_TIMEOUT)
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

/// Resolve detected versions into the engine's portable advisory outcome.
async fn advisory_lookup(detected: &[DetectedLibrary]) -> AdvisoryLookup {
    let packages: Vec<InstalledPackage> = detected
        .iter()
        .map(|lib| InstalledPackage {
            name: lib.name.clone(),
            version: lib.version.clone(),
            ecosystem: Ecosystem::Npm,
            source: "page-script".to_string(),
            is_dev: false,
            workspace_members: Vec::new(),
        })
        .collect();

    let vulns = osv::check_vulnerabilities(&packages).await.vulns;
    if vulns.is_empty() && !osv_reachable().await {
        return AdvisoryLookup::Unavailable;
    }
    AdvisoryLookup::Answered(
        vulns
            .into_iter()
            .map(|vuln| LibraryAdvisory {
                package_name: vuln.package_name,
                current_version: vuln.current_version,
                advisory_id: vuln.advisory_id,
                severity: vuln.severity.as_str().to_string(),
                advisory_url: vuln.advisory_url,
                fixed_version: None,
            })
            .collect(),
    )
}

#[async_trait::async_trait]
impl AsyncCheck for VulnerableLibrariesCheck {
    fn id(&self) -> &str {
        "security.vulnerable_libraries"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let detected = detect_libraries(&ctx.body);
        if detected.is_empty() {
            // Nothing detectable: stay silent rather than claim a clean bill,
            // and skip the network round-trip entirely.
            return vec![];
        }
        let lookup = advisory_lookup(&detected).await;
        evaluate_vulnerable_libraries(&detected, lookup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(body: &str) -> CheckContext {
        CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("https://example.com/page").expect("static test url"),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: body.to_string(),
                is_localhost: false,
                is_strict_localhost: false,
                http_version: Some("HTTP/2.0".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(false).clone(),
            probe_cache: Default::default(),
        }
    }

    #[tokio::test]
    async fn a_page_without_detectable_libraries_emits_nothing_and_asks_nobody() {
        // No detection means no OSV round-trip at all, so this test stays
        // offline-safe while pinning the silent-not-clean-bill behavior.
        let results = VulnerableLibrariesCheck
            .run(&ctx(r#"<script src="/js/bundle.min.js"></script>"#))
            .await;
        assert!(results.is_empty());
    }
}
