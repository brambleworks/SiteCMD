use super::{
    chunk_request_script, decode_bridged_payload, parse_bridge_frame, poll_webview, transfer,
    BridgeFrame, BridgeReadError, TitleBridge, TITLE_BRIDGE_CHUNK_CHARS,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use std::time::Duration;

/// WebKit truncates every document title to this many UTF-16 units before the
/// UI process sees it; Chromium refuses anything past 4096 as a bad message.
const WEBKIT_TITLE_CAP: usize = 1000;

const AXE_MARKER: &str = "___SHK_AXE___";
const CWV_MARKER: &str = "___SHK_CWV___";

#[tokio::test(start_paused = true)]
async fn poll_webview_returns_immediately_when_probe_is_ready() {
    let start = tokio::time::Instant::now();
    let result = poll_webview(Duration::from_millis(100), Duration::from_secs(8), || {
        Some(42)
    })
    .await;
    assert_eq!(result, Some(42));
    assert_eq!(start.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn poll_webview_returns_as_soon_as_probe_succeeds() {
    let start = tokio::time::Instant::now();
    let mut calls = 0;
    let result = poll_webview(Duration::from_millis(100), Duration::from_secs(8), || {
        calls += 1;
        (calls >= 5).then_some(())
    })
    .await;
    assert_eq!(result, Some(()));
    // 5th probe fires after 4 sleeps: 400ms, nowhere near the 8s cap.
    assert_eq!(start.elapsed(), Duration::from_millis(400));
}

#[tokio::test(start_paused = true)]
async fn poll_webview_gives_up_at_the_cap() {
    let start = tokio::time::Instant::now();
    let result = poll_webview(Duration::from_millis(100), Duration::from_secs(1), || {
        None::<()>
    })
    .await;
    assert_eq!(result, None);
    assert!(start.elapsed() <= Duration::from_secs(1));
    assert!(start.elapsed() >= Duration::from_millis(900));
}

#[test]
fn title_bridge_hands_back_only_the_marker_it_is_asked_for() {
    let titles = TitleBridge::default();
    assert_eq!(titles.read_prefixed("___SHK_READY___"), None);

    titles.record("SiteCMD Analyzer".to_string());
    assert_eq!(titles.read_prefixed("___SHK_READY___"), None);

    titles.record("___SHK_READY___".to_string());
    assert_eq!(titles.read_prefixed("___SHK_READY___"), Some(String::new()));
    // A stale marker never satisfies a later poll for a different one.
    assert_eq!(titles.read_prefixed(CWV_MARKER), None);

    titles.record(format!("{CWV_MARKER}0/1:e30="));
    assert_eq!(
        titles.read_prefixed(CWV_MARKER).as_deref(),
        Some("0/1:e30=")
    );
    assert_eq!(titles.read_prefixed(AXE_MARKER), None);

    let shared = titles.clone();
    shared.record(format!("{AXE_MARKER}pending"));
    assert_eq!(titles.read_prefixed(AXE_MARKER).as_deref(), Some("pending"));
}

#[test]
fn frames_parse_index_total_and_data() {
    // Base64 data may itself contain '/', so the header ends at the colon.
    let frame = parse_bridge_frame("3/57:QUJD/+==").expect("frame");
    assert_eq!(
        frame,
        BridgeFrame {
            index: 3,
            total: 57,
            data: "QUJD/+==".to_string()
        }
    );
    let empty = parse_bridge_frame("0/1:").expect("empty data frame");
    assert_eq!(empty.data, "");

    assert_eq!(parse_bridge_frame("pending"), None);
    assert_eq!(parse_bridge_frame("57/57:x"), None, "index past total");
    assert_eq!(parse_bridge_frame("0/0:x"), None, "zero total");
    assert_eq!(parse_bridge_frame("a/b:c"), None);
    assert_eq!(parse_bridge_frame("3:57/x"), None);
    assert_eq!(parse_bridge_frame(""), None);
}

#[test]
fn chunk_requests_fit_under_the_webkit_title_cap() {
    for marker in [AXE_MARKER, CWV_MARKER] {
        let widest_header = format!("{marker}{i}/{n}:", i = 999_999, n = 999_999);
        assert!(
            widest_header.len() + TITLE_BRIDGE_CHUNK_CHARS <= WEBKIT_TITLE_CAP,
            "{marker}: header {} + chunk {TITLE_BRIDGE_CHUNK_CHARS} must fit in {WEBKIT_TITLE_CAP}",
            widest_header.len()
        );
    }
}

#[test]
fn chunk_request_script_wraps_the_page_function_with_its_arguments() {
    let script = chunk_request_script("__SHK_AXE__", AXE_MARKER, 3);
    assert!(
        script.ends_with(&format!(
            ")('__SHK_AXE__', '{AXE_MARKER}', 3, {TITLE_BRIDGE_CHUNK_CHARS})"
        )),
        "arguments applied to the page function: {script}"
    );
    assert!(
        script.starts_with("((function ("),
        "page function stays an expression"
    );
    assert!(
        !script.contains(";)"),
        "the file's statement terminator must not survive inside the call"
    );
    assert!(script.contains("document.title = marker + \"pending\""));
    assert!(
        script.contains("btoa("),
        "chunks are base64 so no whitespace or backslash reaches the title"
    );
}

#[test]
fn bridged_payloads_reassemble_utf8_from_base64_chunks() {
    let json = "{\"html\":\"Caf\u{e9}   cr\u{e8}me \u{1f389}\"}";
    let encoded = STANDARD.encode(json);
    let (head, tail) = encoded.split_at(7);
    let mut assembled = String::new();
    assembled.push_str(head);
    assembled.push_str(tail);
    assert_eq!(decode_bridged_payload(&assembled).as_deref(), Ok(json));

    assert!(matches!(
        decode_bridged_payload("not*base64"),
        Err(BridgeReadError::Undecodable(_))
    ));
    assert!(matches!(
        decode_bridged_payload(&STANDARD.encode([0xff, 0xfe])),
        Err(BridgeReadError::Undecodable(_))
    ));
}

/// A page that answers every chunk request instantly, the way the real one
/// does once its value exists.
fn instant_page<'a>(
    titles: &'a TitleBridge,
    marker: &'static str,
    chunks: Vec<String>,
) -> impl FnMut(usize) + 'a {
    let total = chunks.len();
    move |index| {
        titles.record(format!("{marker}{index}/{total}:{}", chunks[index]));
    }
}

fn chunked(json: &str, size: usize) -> Vec<String> {
    let encoded = STANDARD.encode(json);
    encoded
        .as_bytes()
        .chunks(size)
        .map(|chunk| String::from_utf8(chunk.to_vec()).expect("base64 is ascii"))
        .collect()
}

#[tokio::test(start_paused = true)]
async fn transfer_requests_every_chunk_in_order_and_decodes_the_result() {
    let json = "{\"violations\":[{\"id\":\"color-contrast\",\"html\":\"<p>\u{e9}</p>\"}]}";
    let titles = TitleBridge::default();
    let requested = std::cell::RefCell::new(Vec::new());
    let mut page = instant_page(&titles, AXE_MARKER, chunked(json, 8));
    let result = transfer(
        &titles,
        AXE_MARKER,
        Duration::from_millis(250),
        Duration::from_secs(20),
        |index| {
            requested.borrow_mut().push(index);
            page(index);
        },
    )
    .await;
    assert_eq!(result.as_deref(), Ok(json));
    let expected: Vec<usize> = (0..chunked(json, 8).len()).collect();
    assert_eq!(*requested.borrow(), expected);
}

#[tokio::test(start_paused = true)]
async fn transfer_reports_a_value_that_never_appears() {
    let titles = TitleBridge::default();
    let start = tokio::time::Instant::now();
    let result = transfer(
        &titles,
        AXE_MARKER,
        Duration::from_millis(250),
        Duration::from_secs(20),
        |_| titles.record(format!("{AXE_MARKER}pending")),
    )
    .await;
    assert!(matches!(result, Err(BridgeReadError::NotReady)));
    assert!(start.elapsed() >= Duration::from_secs(19));
    assert!(start.elapsed() <= Duration::from_secs(20));
}

#[tokio::test(start_paused = true)]
async fn transfer_reports_the_chunk_a_stalled_page_never_served() {
    let json = "{\"passes\":[\"a\",\"b\",\"c\",\"d\",\"e\",\"f\",\"g\"]}";
    let chunks = chunked(json, 16);
    assert!(chunks.len() >= 3, "test needs several chunks");
    let titles = TitleBridge::default();
    let total = chunks.len();
    let result = transfer(
        &titles,
        AXE_MARKER,
        Duration::from_millis(250),
        Duration::from_secs(20),
        |index| {
            if index < 2 {
                titles.record(format!("{AXE_MARKER}{index}/{total}:{}", chunks[index]));
            }
        },
    )
    .await;
    assert!(
        matches!(result, Err(BridgeReadError::Stalled { index: 2, total: t }) if t == total),
        "{result:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn transfer_re_requests_a_chunk_the_page_overwrote() {
    // A page that keeps rewriting its own title (a notification ticker, a
    // router) can clobber a frame before the poller reads it; the poller asks
    // again rather than giving up on that chunk.
    let json = "{\"incomplete\":[\"landmark-one-main\",\"region\"]}";
    let chunks = chunked(json, 12);
    let total = chunks.len();
    let titles = TitleBridge::default();
    let mut asked_for_one = 0;
    let result = transfer(
        &titles,
        AXE_MARKER,
        Duration::from_millis(250),
        Duration::from_secs(20),
        |index| {
            if index == 1 {
                asked_for_one += 1;
                if asked_for_one == 1 {
                    titles.record("(3) New messages".to_string());
                    return;
                }
            }
            titles.record(format!("{AXE_MARKER}{index}/{total}:{}", chunks[index]));
        },
    )
    .await;
    assert_eq!(result.as_deref(), Ok(json));
    assert_eq!(asked_for_one, 2);
}

#[test]
fn transfer_errors_read_as_plain_sentences() {
    assert_eq!(
        BridgeReadError::NotReady.to_string(),
        "the page never produced a value to read"
    );
    assert_eq!(
        BridgeReadError::Stalled { index: 2, total: 9 }.to_string(),
        "the page stopped answering at chunk 3 of 9"
    );
    assert_eq!(
        BridgeReadError::Undecodable("bad base64".to_string()).to_string(),
        "the transferred payload could not be decoded: bad base64"
    );
}
