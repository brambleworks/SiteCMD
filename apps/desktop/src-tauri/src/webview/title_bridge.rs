//! Document-title bridge between the analyzer and the page it loads.
//!
//! External pages cannot reach Tauri's IPC, so every read-back (readiness,
//! browser build, Core Web Vitals, axe) travels through `document.title`.
//! That channel is narrow: WebKit truncates a title to 1000 UTF-16 units
//! before the UI process sees it, collapses whitespace runs, and delivers
//! only the last of a burst of changes, while Chromium refuses a title past
//! 4096 units as a bad message. A JSON payload therefore crosses as base64
//! chunks that the analyzer requests one index at a time. The page answers
//! each request with the frame `<marker><index>/<total>:<data>`, or
//! `<marker>pending` until it has a value, and [`transfer`] reassembles the
//! frames here.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Latest `document.title` the platform webview reported. Tauri only mirrors
/// that into the window title when a handler asks it to, and
/// `WebviewWindow::title()` otherwise returns the static window title for the
/// life of the scan, so the pollers read this bridge instead.
#[derive(Clone, Default)]
pub(crate) struct TitleBridge(Arc<Mutex<Option<String>>>);

impl TitleBridge {
    pub(crate) fn record(&self, title: String) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(title);
        }
    }

    /// The current title with `prefix` removed, or `None` while the title
    /// carries some other marker or nothing at all.
    pub(crate) fn read_prefixed(&self, prefix: &str) -> Option<String> {
        self.0
            .lock()
            .ok()?
            .as_deref()?
            .strip_prefix(prefix)
            .map(str::to_string)
    }
}

/// Poll immediately and then at `interval` until a value arrives or `cap` elapses.
pub(crate) async fn poll_webview<T>(
    interval: Duration,
    cap: Duration,
    mut probe: impl FnMut() -> Option<T>,
) -> Option<T> {
    let deadline = tokio::time::Instant::now() + cap;
    loop {
        if let Some(value) = probe() {
            return Some(value);
        }
        if tokio::time::Instant::now() + interval > deadline {
            return None;
        }
        tokio::time::sleep(interval).await;
    }
}

/// Base64 characters per chunk. Together with the widest possible frame
/// header this stays under WebKit's 1000-unit title cap; a test pins the sum.
pub(crate) const TITLE_BRIDGE_CHUNK_CHARS: usize = 900;

const CHUNK_REQUEST_FUNCTION: &str = include_str!("title_bridge.js");

/// The script that asks the page for chunk `index` of the JSON value parked
/// in `global`, framed under `marker`.
pub(crate) fn chunk_request_script(global: &str, marker: &str, index: usize) -> String {
    // The file is a bare function expression with a leading comment and a
    // statement terminator; only the expression goes inside the call.
    let function = CHUNK_REQUEST_FUNCTION
        .lines()
        .skip_while(|line| {
            let line = line.trim();
            line.is_empty() || line.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let function = function.trim_end().trim_end_matches(';');
    format!("({function})('{global}', '{marker}', {index}, {TITLE_BRIDGE_CHUNK_CHARS})")
}

/// One answered chunk request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BridgeFrame {
    pub(crate) index: usize,
    pub(crate) total: usize,
    pub(crate) data: String,
}

/// Parses the title body after the marker. Base64 data may contain `/`, so
/// the header is everything before the first colon.
pub(crate) fn parse_bridge_frame(body: &str) -> Option<BridgeFrame> {
    let (header, data) = body.split_once(':')?;
    let (index, total) = header.split_once('/')?;
    let index: usize = index.parse().ok()?;
    let total: usize = total.parse().ok()?;
    if total == 0 || index >= total {
        return None;
    }
    Some(BridgeFrame {
        index,
        total,
        data: data.to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BridgeReadError {
    /// The page never parked a value within the caller's readiness budget.
    NotReady,
    /// The value existed but one chunk never arrived after being requested.
    Stalled { index: usize, total: usize },
    /// Every chunk arrived but the reassembled bytes were not base64 UTF-8.
    Undecodable(String),
}

impl std::fmt::Display for BridgeReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady => write!(f, "the page never produced a value to read"),
            Self::Stalled { index, total } => write!(
                f,
                "the page stopped answering at chunk {} of {total}",
                index + 1
            ),
            Self::Undecodable(reason) => {
                write!(f, "the transferred payload could not be decoded: {reason}")
            }
        }
    }
}

pub(crate) fn decode_bridged_payload(encoded: &str) -> Result<String, BridgeReadError> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| BridgeReadError::Undecodable(error.to_string()))?;
    String::from_utf8(bytes).map_err(|error| BridgeReadError::Undecodable(error.to_string()))
}

/// Runs the chunk protocol against `request`, which asks the page for one
/// chunk index; the page answers through `titles`. The first chunk waits on
/// the caller's readiness budget because the page may still be computing the
/// value. Later chunks use the tight transfer budget: the value exists and
/// only the eval round trip remains. Every poll that finds no matching frame
/// asks again, so a page that rewrites its own title mid-transfer costs a
/// retry rather than the whole payload.
pub(crate) async fn transfer(
    titles: &TitleBridge,
    marker: &str,
    ready_poll: Duration,
    ready_cap: Duration,
    mut request: impl FnMut(usize),
) -> Result<String, BridgeReadError> {
    let mut encoded = String::new();
    let mut index = 0;
    let mut total = 1;
    while index < total {
        let (interval, cap) = if index == 0 {
            (ready_poll, ready_cap)
        } else {
            (
                crate::constants::TITLE_BRIDGE_CHUNK_POLL_INTERVAL,
                crate::constants::TITLE_BRIDGE_CHUNK_TIMEOUT,
            )
        };
        let frame = poll_webview(interval, cap, || {
            let frame = titles
                .read_prefixed(marker)
                .and_then(|body| parse_bridge_frame(&body))
                .filter(|frame| frame.index == index);
            if frame.is_none() {
                request(index);
            }
            frame
        })
        .await;
        let Some(frame) = frame else {
            return Err(if index == 0 {
                BridgeReadError::NotReady
            } else {
                BridgeReadError::Stalled { index, total }
            });
        };
        total = frame.total;
        encoded.push_str(&frame.data);
        index += 1;
    }
    decode_bridged_payload(&encoded)
}

/// Reads the JSON value the page parked in `global` through the title bridge.
pub(crate) async fn read_bridged_json(
    webview: &tauri::WebviewWindow,
    titles: &TitleBridge,
    global: &str,
    marker: &str,
    ready_poll: Duration,
    ready_cap: Duration,
) -> Result<String, BridgeReadError> {
    transfer(titles, marker, ready_poll, ready_cap, |index| {
        let _ = webview.eval(chunk_request_script(global, marker, index));
    })
    .await
}

#[cfg(test)]
#[path = "title_bridge_tests.rs"]
mod tests;
