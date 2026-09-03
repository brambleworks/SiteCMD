/// The throttle has to be off before the page loads: axe-core and the page
/// under measurement both run on `setTimeout`, and a hidden page's timers
/// otherwise land on WebKit's widening one-second grid.
#[test]
fn analyzer_unthrottles_the_page_before_it_navigates() {
    let source = include_str!("analyzer.rs");
    let unthrottle = source
        .find("unthrottle_hidden_page_timers(webview)")
        .expect("analyzer switches the hidden-page throttle off");
    let navigate = source
        .find("webview.navigate(target)")
        .expect("analyzer navigates to the target");
    assert!(
        unthrottle < navigate,
        "the throttle must be off before the target page starts loading"
    );
}

/// The WebKit setter is private API. A future WebKit may drop it, and an
/// unrecognised selector would abort the process, so the arm must ask first.
#[test]
fn the_mac_arm_checks_for_the_private_setter_before_calling_it() {
    let source = include_str!("hidden_page_timers.rs");
    let check = source
        .find("respondsToSelector: selector")
        .expect("mac arm checks the selector");
    let call = source
        .find("_setHiddenPageDOMTimerThrottlingEnabled: false")
        .expect("mac arm switches the throttle off");
    assert!(
        check < call,
        "respondsToSelector must guard the private setter"
    );
}

/// WebView2 reads browser flags only when the first webview creates the
/// shared browser process, so the flag rides on the main window's config.
/// Overriding the args also drops wry's defaults, which have to come along.
#[test]
fn windows_passes_the_browser_flag_when_the_shared_process_starts() {
    let config: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json"))
        .expect("tauri.conf.json parses");
    let windows = config["app"]["windows"]
        .as_array()
        .expect("app.windows is a list");
    assert!(!windows.is_empty());
    for window in windows {
        let args = window["additionalBrowserArgs"]
            .as_str()
            .expect("every configured window carries additionalBrowserArgs");
        assert!(
            args.contains("--disable-background-timer-throttling"),
            "hidden analyzer pages must not be timer-throttled on Windows: {args}"
        );
        assert!(
            args.contains("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection"),
            "overriding the args must keep wry's default feature disables: {args}"
        );
    }
}
