//! Desktop transport for portable HTTPS-enforcement probes.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::https_enforcement::{
    evaluate_http_downgrade, evaluate_https_availability, origin_root_request,
    plan_https_enforcement, HttpsEnforcementStep,
};

pub struct HttpsEnforcementCheck;

#[async_trait::async_trait]
impl AsyncCheck for HttpsEnforcementCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.https_enforcement"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        // Each step names its own grader; the pairing is not inferred from
        // the probe URL, so a future edit cannot quietly grade the HTTPS
        // availability probe as a missing downgrade redirect or the reverse.
        match plan_https_enforcement(&ctx.url, ctx.is_localhost) {
            HttpsEnforcementStep::Done(results) => results,
            HttpsEnforcementStep::ProbeHttpOrigin { url: http_origin } => {
                let outcome = probe(&ctx.client, origin_root_request(&http_origin)).await;
                evaluate_http_downgrade(http_origin.as_str(), outcome)
            }
            HttpsEnforcementStep::ProbeHttpsOrigin { url: https_origin } => {
                let outcome = probe(&ctx.client, origin_root_request(&https_origin)).await;
                evaluate_https_availability(https_origin.as_str(), outcome)
            }
        }
    }

    fn skip_in_predeploy(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    /// A context built the way `core::scanner::fetch_page` builds one, so the
    /// local-environment flags come from the same authority the check reads.
    fn ctx_for(url: &str) -> CheckContext {
        let parsed = url::Url::parse(url).expect("static test url");
        CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                is_localhost: crate::core::localhost::is_localhost(&parsed),
                is_strict_localhost: crate::core::localhost::is_strict_localhost(&parsed),
                url: parsed,
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: String::new(),
                http_version: Some("HTTP/2.0".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(false).clone(),
            probe_cache: Default::default(),
        }
    }

    #[tokio::test]
    async fn a_local_http_scan_is_skipped_without_any_request() {
        // Loopback and the dev-host names `core::localhost` recognizes must
        // reach the same skip, because a `.ddev.site` or `.test` page is a
        // preview server too and the neighbouring checks already treat it so.
        for target in [
            "http://127.0.0.1:4321/page",
            "http://localhost:4321/",
            "http://my-app.ddev.site/",
            "http://shop.test/checkout",
            // A dev server on the LAN, recognized by address range because it
            // has no hostname to match.
            "http://192.168.1.40:8080/",
        ] {
            let results = HttpsEnforcementCheck.run(&ctx_for(target)).await;
            assert_eq!(results[0].status, CheckStatus::Skipped, "{target}");
            assert!(
                results[0].description.contains("localhost preview"),
                "{target}"
            );
        }
    }

    #[test]
    fn a_private_address_is_local_by_range_while_a_public_one_is_still_probed() {
        // Name matching cannot see a LAN dev server, so this check used to
        // plan an HTTPS probe against whatever answers 443 on that address
        // and grade a preview box like a production site. `is_localhost` now
        // reads the address ranges, which closes the same gap in
        // config.custom_404 and security.cors_reflection.
        let lan = url::Url::parse("http://192.168.1.40:8080/").expect("static test url");
        assert!(crate::core::localhost::is_localhost(&lan));
        assert!(matches!(
            plan_https_enforcement(&lan, crate::core::localhost::is_localhost(&lan)),
            HttpsEnforcementStep::Done(_)
        ));

        // Negative control: a public literal is not swept up by the ranges.
        let public = url::Url::parse("http://93.184.216.34/").expect("static test url");
        assert!(!crate::core::localhost::is_localhost(&public));
        assert!(matches!(
            plan_https_enforcement(&public, crate::core::localhost::is_localhost(&public)),
            HttpsEnforcementStep::ProbeHttpsOrigin { .. }
        ));
    }

    #[tokio::test]
    async fn a_public_http_scan_whose_https_probe_does_not_answer_still_fails() {
        // The scanned page arrived over cleartext, which is the defect; the
        // reserved-TLD host cannot answer the HTTPS probe, which is the
        // "no HTTPS response observed" wording. Nothing here asks anything of
        // the network beyond the resolver saying no (RFC 2606).
        let results = HttpsEnforcementCheck
            .run(&ctx_for("http://sitecmd-unreachable.invalid/page"))
            .await;
        assert_eq!(results[0].status, crate::checks::CheckStatus::Fail);
        assert_eq!(
            results[0].title,
            "No HTTPS response observed; site served over HTTP"
        );
    }

    #[tokio::test]
    async fn an_unreachable_http_origin_is_skipped_not_failed() {
        let results = HttpsEnforcementCheck
            .run(&ctx_for("https://sitecmd-unreachable.invalid/page"))
            .await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("could not obtain"));
    }
}
