//! Adapts the shared redirect walk to the temporary-redirect verdict.

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::seo::redirects::evaluate_temporary_redirect;

pub struct TemporaryRedirectCheck;

#[async_trait::async_trait]
impl AsyncCheck for TemporaryRedirectCheck {
    fn id(&self) -> &str {
        "seo.temporary_redirect"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        vec![evaluate_temporary_redirect(ctx.redirect_chain().await)]
    }

    fn skip_in_predeploy(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::checks::{
        AsyncCheck, CheckContext, CheckStatus, RedirectHop, RedirectWalk, RedirectWalkTermination,
    };

    #[tokio::test]
    async fn shell_grades_the_seeded_walk_through_the_engine() {
        // The canonicalization taxonomy is pinned by the engine tests and
        // corpus; this proves the shell hands the shared walk over.
        let ctx = CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("http://example.com/").unwrap(),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: String::new(),
                is_localhost: false,
                is_strict_localhost: false,
                http_version: Some("HTTP/2.0".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(false).clone(),
            probe_cache: Default::default(),
        };
        assert!(ctx
            .probe_cache
            .redirect_chain
            .set(RedirectWalk {
                hops: vec![RedirectHop {
                    from: "http://example.com/".into(),
                    to: "https://example.com/".into(),
                    status: 302,
                }],
                termination: RedirectWalkTermination::FinalResponse {
                    url: "https://example.com/".into(),
                    status: 200,
                },
            })
            .is_ok());
        let results = super::TemporaryRedirectCheck.run(&ctx).await;
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("HTTP 302"));
    }
}
