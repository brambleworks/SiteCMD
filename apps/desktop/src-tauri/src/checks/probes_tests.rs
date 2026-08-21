use crate::checks::{CheckContext, RobotsTxtFetch, SitemapProbe};

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

// One-shot HTTP server that answers every accepted connection with the
// given response and returns the origin to probe.
fn serve(response: &'static str, connections: usize) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let origin = format!("http://{}", listener.local_addr().expect("listener addr"));
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..connections {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    origin
}

#[tokio::test]
async fn robots_fetch_through_the_seam_classifies_a_real_body_as_found() {
    let origin = serve(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 24\r\nConnection: close\r\n\r\nUser-agent: *\nDisallow:\n",
        1,
    );
    let ctx = ctx_for(&format!("{origin}/page"));
    match ctx.robots_txt().await {
        RobotsTxtFetch::Found { body } => assert!(body.contains("User-agent")),
        other => panic!("expected Found, got {other:?}"),
    }
}

#[tokio::test]
async fn robots_html_catch_all_classifies_as_error_not_empty_robots() {
    let origin = serve(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 15\r\nConnection: close\r\n\r\n<!doctype html>",
        1,
    );
    let ctx = ctx_for(&format!("{origin}/page"));
    assert!(matches!(ctx.robots_txt().await, RobotsTxtFetch::Error(_)));
}

#[tokio::test]
async fn unreachable_origin_yields_an_inconclusive_sitemap_probe() {
    // Closed loopback port: every candidate fails at the transport, which
    // must classify as Inconclusive, never Missing.
    let ctx = ctx_for("http://127.0.0.1:1/");
    assert!(matches!(
        ctx.sitemap().await,
        SitemapProbe::Inconclusive { .. }
    ));
}
