//! Transport-sequencing tests for the link probe shell: the HEAD-then-GET
//! two-step against a real local server. The collection, resolution, and
//! verdict tests live with the engine module.

use super::*;
use sitecmd_engine::checks::seo::links::ProbeOutcomeKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn method_status_server(
    head_status: u16,
    get_status: u16,
) -> (url::Url, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind link probe server");
    let address = listener.local_addr().expect("test server address");
    let handle = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.expect("accept probe");
            let mut request = [0u8; 2048];
            let read = stream.read(&mut request).await.expect("read probe");
            let method = String::from_utf8_lossy(&request[..read])
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            let status = if method == "HEAD" {
                head_status
            } else {
                get_status
            };
            let reason = match status {
                200 => "OK",
                404 => "Not Found",
                410 => "Gone",
                500 => "Internal Server Error",
                _ => "Response",
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write probe response");
        }
    });
    (
        url::Url::parse(&format!("http://{address}/target")).unwrap(),
        handle,
    )
}

async fn observe(head_status: u16, get_status: u16) -> LinkObservation {
    let (url, server) = method_status_server(head_status, get_status).await;
    let observation = probe_one_link(
        crate::http_client::for_url(true).clone(),
        url,
        crate::constants::CHECK_PROBE_TIMEOUT,
    )
    .await;
    server.await.expect("server task");
    observation
}

#[tokio::test]
async fn get_success_overrides_a_head_404() {
    // Some frameworks do not implement HEAD faithfully, so an error HEAD
    // always earns one GET confirmation before anything is called broken.
    assert_eq!(observe(404, 200).await.kind(), ProbeOutcomeKind::Responded);
}

#[tokio::test]
async fn get_404_confirms_a_missing_destination() {
    assert_eq!(observe(404, 404).await.kind(), ProbeOutcomeKind::Missing);
}

#[tokio::test]
async fn get_500_remains_inconclusive() {
    assert_eq!(
        observe(500, 500).await.kind(),
        ProbeOutcomeKind::Inconclusive
    );
}
