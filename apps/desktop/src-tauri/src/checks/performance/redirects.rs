//! Desktop transport for the shared redirect walk used by performance and SEO.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, RedirectWalk, ScanCategory};
use sitecmd_engine::checks::performance::redirects::{
    evaluate_redirect_chain, RedirectWalkStep, RedirectWalker,
};
use sitecmd_engine::probe::{ProbeFailure, ProbeFailureClass, ProbeOutcome, ProbeRequest};

/// Follow redirects from the scanned URL without auto-following, one seam
/// probe per hop, until the engine walker declares the walk complete.
/// Called once per scan via `CheckContext::redirect_chain`.
pub(crate) async fn walk_redirect_chain(ctx: &CheckContext) -> RedirectWalk {
    walk_redirect_chain_with_probe(ctx.requested_url(), ctx.subordinate_policy(), |request| {
        probe(&ctx.client, request)
    })
    .await
}

async fn walk_redirect_chain_with_probe<F, Fut>(
    start_url: &url::Url,
    policy: crate::network_policy::UrlPolicy,
    mut execute_probe: F,
) -> RedirectWalk
where
    F: FnMut(ProbeRequest) -> Fut,
    Fut: std::future::Future<Output = ProbeOutcome>,
{
    let mut walker = RedirectWalker::new(start_url);
    loop {
        let request = walker.request();
        let allowed = url::Url::parse(&request.url).ok().and_then(|target| {
            crate::network_policy::validate_page_subresource_target(&target, policy).ok()
        });
        let outcome = if allowed.is_some() {
            execute_probe(request).await
        } else {
            ProbeOutcome::Failure(ProbeFailure {
                class: ProbeFailureClass::Transport,
                detail: "redirect target refused by network policy".into(),
            })
        };
        match walker.observe(&outcome) {
            RedirectWalkStep::Continue(next) => walker = next,
            RedirectWalkStep::Done(walk) => return walk,
        }
    }
}

/// Detects redirect chains (too many hops from the original URL)
pub struct RedirectChainCheck;

#[async_trait::async_trait]
impl AsyncCheck for RedirectChainCheck {
    fn id(&self) -> &str {
        "performance.redirect_chain"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let walk = ctx.redirect_chain().await;
        // Use the requested URL so redirect evidence agrees with the probe lane's
        // starting point rather than reporting the chain's destination.
        vec![evaluate_redirect_chain(ctx.requested_url().as_ref(), walk)]
    }

    fn skip_in_predeploy(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::checks::{CheckContext, RedirectWalkTermination};
    use sitecmd_engine::probe::{ProbeOutcome, ProbeResponse};

    fn ctx_for(url: &str) -> CheckContext {
        CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse(url).expect("static test url"),
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
        }
    }

    fn ctx_for_request(requested_url: &str, effective_url: &str) -> CheckContext {
        ctx_for(effective_url)
            .with_requested_url(url::Url::parse(requested_url).expect("static requested test url"))
    }

    #[tokio::test]
    async fn walk_through_the_seam_records_a_real_hop_and_final_response() {
        // One 302 hop to /done, then a 200: proves the shell loop feeds the
        // engine walker real classified outcomes in sequence.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let origin = format!("http://{}", listener.local_addr().expect("listener addr"));
        let redirect = format!(
            "HTTP/1.1 302 Found\r\nLocation: {origin}/done\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for response in [
                redirect,
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
            ] {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let ctx = ctx_for(&format!("{origin}/start"));
        let walk = ctx.redirect_chain().await;
        assert_eq!(walk.hops.len(), 1);
        assert_eq!(walk.hops[0].status, 302);
        assert!(walk.hops[0].to.ends_with("/done"));
        assert!(matches!(
            walk.termination,
            RedirectWalkTermination::FinalResponse { status: 200, .. }
        ));
    }

    #[tokio::test]
    async fn walk_starts_at_the_requested_url_after_the_primary_fetch_followed_redirects() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let origin = format!("http://{}", listener.local_addr().expect("listener addr"));
        let redirect = format!(
            "HTTP/1.1 302 Found\r\nLocation: {origin}/done\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for response in [
                redirect,
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
            ] {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let ctx = ctx_for_request(&format!("{origin}/start"), &format!("{origin}/done"));
        let walk = ctx.redirect_chain().await;
        assert_eq!(walk.hops.len(), 1);
        assert!(walk.hops[0].from.ends_with("/start"));
        assert!(walk.hops[0].to.ends_with("/done"));
    }

    #[tokio::test]
    async fn unreachable_origin_walk_is_inconclusive() {
        let ctx = ctx_for("http://127.0.0.1:1/");
        let walk = ctx.redirect_chain().await;
        assert!(matches!(
            walk.termination,
            RedirectWalkTermination::NetworkError { .. }
        ));
    }

    #[tokio::test]
    async fn policy_refused_redirect_target_is_never_sent_to_the_transport() {
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let probe_observed = observed.clone();
        let start = url::Url::parse("https://example.com/start").expect("static public URL");

        let public = crate::network_policy::UrlPolicy::Redirect {
            allow_local_dev: false,
        };
        let walk = super::walk_redirect_chain_with_probe(&start, public, move |request| {
            let probe_observed = probe_observed.clone();
            async move {
                probe_observed
                    .lock()
                    .expect("recorded requests lock")
                    .push(request.url.clone());
                ProbeOutcome::Response(ProbeResponse {
                    status: 302,
                    final_url: request.url,
                    content_type: None,
                    content_length: Some(0),
                    headers: vec![(
                        "location".into(),
                        "http://169.254.169.254/latest/meta-data".into(),
                    )],
                    body: None,
                })
            }
        })
        .await;

        assert_eq!(
            *observed.lock().expect("recorded requests lock"),
            vec!["https://example.com/start".to_string()]
        );
        assert!(matches!(
            walk.termination,
            RedirectWalkTermination::NetworkError { ref url }
                if url == "http://169.254.169.254/latest/meta-data"
        ));
    }

    #[tokio::test]
    async fn the_evidence_names_the_requested_url_rather_than_the_landed_one() {
        use super::RedirectChainCheck;
        use crate::checks::AsyncCheck;

        let ctx = ctx_for_request(
            "http://requested.invalid/",
            "http://landed.invalid/elsewhere",
        );
        let results = RedirectChainCheck.run(&ctx).await;
        let raw = results[0]
            .raw_data
            .as_ref()
            .expect("the verdict carries evidence");
        assert_eq!(raw["start_url"], "http://requested.invalid/");
    }
}
