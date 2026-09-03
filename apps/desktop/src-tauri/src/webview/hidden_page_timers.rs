//! Keeps the hidden analyzer page running at full speed.
//!
//! The analyzer window is never shown, and both platform webviews treat a
//! hidden page as a background tab: WebKit aligns its DOM timers to a
//! one-second grid that keeps widening (hidden-page DOM timer throttling),
//! and Chromium applies its background timer throttling. axe-core schedules
//! its rule batches through `setTimeout`, so a run that takes a quarter of a
//! second in a visible page took fifteen in the analyzer, and any page
//! script the Web Vitals observer measures is slowed the same way. On macOS
//! the throttle is a per-page WebKit preference switched off here. On
//! Windows it is a browser-process flag that WebView2 reads only when the
//! first webview creates the shared browser process, so `tauri.conf.json`
//! passes it through `additionalBrowserArgs` on the main window.

/// macOS: switch off WebKit's hidden-page DOM timer throttling for this
/// webview. The setter is a private WKPreferences accessor, so its presence
/// is checked first; a WebKit that dropped it leaves the page throttled
/// rather than failing the scan.
#[cfg(target_os = "macos")]
pub(crate) fn unthrottle_hidden_page_timers(webview: &tauri::WebviewWindow) -> Result<(), String> {
    webview
        .with_webview(|platform| {
            use objc2::runtime::AnyObject;
            use objc2::{msg_send, sel};
            // SAFETY: `inner()` is this window's WKWebView and with_webview
            // runs on the main thread, where WebKit objects may be messaged.
            // `configuration` and `preferences` return objects the webview
            // keeps alive for as long as it exists, and they are used only
            // inside this closure.
            unsafe {
                let webview: *mut AnyObject = platform.inner().cast();
                let configuration: *mut AnyObject = msg_send![webview, configuration];
                let preferences: *mut AnyObject = msg_send![configuration, preferences];
                let selector = sel!(_setHiddenPageDOMTimerThrottlingEnabled:);
                let responds: bool = msg_send![preferences, respondsToSelector: selector];
                if responds {
                    let _: () =
                        msg_send![preferences, _setHiddenPageDOMTimerThrottlingEnabled: false];
                } else {
                    tracing::warn!(
                        "WebKit no longer exposes the hidden-page timer throttle; browser analysis runs throttled"
                    );
                }
            }
        })
        .map_err(|error| error.to_string())
}

/// Windows: nothing per window. The throttle is a Chromium browser-process
/// flag, and `tauri.conf.json` passes `--disable-background-timer-throttling`
/// when the main window creates the shared process.
#[cfg(windows)]
pub(crate) fn unthrottle_hidden_page_timers(_webview: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

/// Other platforms never reach the browser layer (see private_network_rules).
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) fn unthrottle_hidden_page_timers(_webview: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
#[path = "hidden_page_timers_tests.rs"]
mod tests;
