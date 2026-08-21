//! Desktop transport for the engine's open-redirect probe plan.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::open_redirect::{
    evaluate_open_redirect, open_redirect_probes, probe_origin, OpenRedirectSweep,
};
use sitecmd_engine::probe::{ProbeFailure, ProbeFailureClass, ProbeOutcome};

pub struct OpenRedirectCheck;

#[async_trait::async_trait]
impl AsyncCheck for OpenRedirectCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.open_redirect"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        // Keep the complete plan so unanswered tasks count against coverage.
        let planned = open_redirect_probes(&probe_origin(&ctx.url));
        // Build the full futures list once so the probes run concurrently.
        let futures: Vec<_> = planned
            .iter()
            .cloned()
            .map(|probed| {
                let client = ctx.client.clone();
                tokio::spawn(async move { probe(&client, probed.request()).await })
            })
            .collect();

        let mut sweep = OpenRedirectSweep::default();
        for (probed, future) in planned.iter().zip(futures) {
            // A join error means the task produced no outcome, which leaves
            // the probe in exactly the standing of one that failed: planned,
            // unanswered, and not evidence of anything.
            let outcome = future.await.unwrap_or_else(|_| {
                ProbeOutcome::Failure(ProbeFailure {
                    class: ProbeFailureClass::Transport,
                    detail: "the probe task did not complete".into(),
                })
            });
            sweep.observe(probed, &outcome);
        }
        evaluate_open_redirect(sweep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    #[tokio::test]
    async fn an_unreachable_origin_declines_to_grade_instead_of_passing() {
        let ctx = CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("http://127.0.0.1:1/").expect("static test url"),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: String::new(),
                is_localhost: true,
                is_strict_localhost: true,
                http_version: Some("HTTP/1.1".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(true).clone(),
            probe_cache: Default::default(),
        };
        let results = OpenRedirectCheck.run(&ctx).await;
        assert_ne!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].title.contains("did not complete"));
        assert_eq!(
            results[0].raw_data.as_ref().expect("raw data")["probes_answered"],
            0
        );
    }
}
