//! Private-network subresource rules for the hidden analyzer webview.
//!
//! The top-level navigation gate in `analyzer.rs` covers the page itself;
//! these rules cover what the page loads: images, scripts, stylesheets,
//! fetch, XHR, and WebSocket targets. IP literals and local names are
//! decided here. A public hostname that resolves to a private address is not
//! covered by either platform filter, WebRTC gathers candidates outside the
//! resource loader that both filters sit on, and Linux has no filter at all;
//! all three are documented gaps the privacy copy must keep stating.

use std::sync::{Arc, Mutex};

/// Identifier under which WebKit stores the compiled rule list.
/// `compileContentRuleList(forIdentifier:)` always recompiles and overwrites
/// whatever that identifier held (only `lookUpContentRuleList` returns a
/// cached list), so two analyzer webviews compiling concurrently under one
/// identifier would overwrite each other's document. The two `allow_local_dev`
/// modes compile different documents and therefore get their own identifiers.
/// Bump the version whenever the patterns change.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const PRIVATE_NETWORK_RULES_IDENTIFIER_STRICT: &str = "sitecmd-private-network-v1-strict";

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const PRIVATE_NETWORK_RULES_IDENTIFIER_LOCAL_DEV: &str = "sitecmd-private-network-v1-local-dev";

#[derive(Debug, Clone, Copy)]
pub(crate) struct PrivateNetworkRules {
    /// A scan of an explicit loopback origin must still load its own assets.
    pub allow_local_dev: bool,
}

/// Host patterns in the regex subset WebKit's `url-filter` dialect and the
/// `regex` crate share: literals, character classes, groups, `*`, `+`, `?`,
/// and anchors. WebKit rejects alternation, so every numeric range is spelled
/// as its own pattern. Tests evaluate them with `regex` as the stand-in for
/// WebKit.
///
/// A regex cannot decode the IPv4 address embedded in an IPv6 literal, so the
/// transition prefixes are blocked whole: every `2002::/16` 6to4, every
/// `2001:0::/32` Teredo, every `64:ff9b::/32` NAT64, and every `::`-prefixed
/// literal is refused rather than only the ones carrying a private address.
/// The bare `[::1]` loopback form is the one exception, because it stays
/// gated by `LOOPBACK_HOSTS`. Blocking a Teredo or 6to4 subresource that
/// happens to embed a public address is not a realistic loss. All of this is
/// stricter than `network_policy`, never looser.
const ALWAYS_BLOCKED_HOSTS: &[(&str, &str)] = &[
    ("rfc1918-10", r"10\.[0-9]+\.[0-9]+\.[0-9]+"),
    ("rfc1918-172-16", r"172\.1[6-9]\.[0-9]+\.[0-9]+"),
    ("rfc1918-172-20", r"172\.2[0-9]\.[0-9]+\.[0-9]+"),
    ("rfc1918-172-30", r"172\.3[01]\.[0-9]+\.[0-9]+"),
    ("rfc1918-192", r"192\.168\.[0-9]+\.[0-9]+"),
    ("link-local", r"169\.254\.[0-9]+\.[0-9]+"),
    ("this-network", r"0\.[0-9]+\.[0-9]+\.[0-9]+"),
    ("cgnat-64", r"100\.6[4-9]\.[0-9]+\.[0-9]+"),
    ("cgnat-70", r"100\.[7-9][0-9]\.[0-9]+\.[0-9]+"),
    ("cgnat-100", r"100\.1[01][0-9]\.[0-9]+\.[0-9]+"),
    ("cgnat-120", r"100\.12[0-7]\.[0-9]+\.[0-9]+"),
    ("ietf-protocol", r"192\.0\.0\.[0-9]+"),
    ("benchmark", r"198\.1[89]\.[0-9]+\.[0-9]+"),
    ("multicast-224", r"22[4-9]\.[0-9]+\.[0-9]+\.[0-9]+"),
    ("multicast-230", r"23[0-9]\.[0-9]+\.[0-9]+\.[0-9]+"),
    ("reserved-240", r"24[0-9]\.[0-9]+\.[0-9]+\.[0-9]+"),
    ("reserved-250", r"25[0-5]\.[0-9]+\.[0-9]+\.[0-9]+"),
    ("v6-unique-local", r"\[f[cd][0-9a-f]*:[^\]]*\]"),
    ("v6-link-local", r"\[fe[89ab][0-9a-f]:[^\]]*\]"),
    ("v6-site-local", r"\[fe[cdef][0-9a-f]:[^\]]*\]"),
    ("v6-mapped", r"\[::ffff:[0-9a-f.:]+\]"),
    ("v6-unspecified", r"\[::\]"),
    ("v6-compatible", r"\[::[02-9a-f][^\]]*\]"),
    ("v6-compatible-past-one", r"\[::1[0-9a-f:][^\]]*\]"),
    ("v6-nat64", r"\[64:ff9b:[^\]]*\]"),
    ("v6-6to4", r"\[2002:[^\]]*\]"),
    ("v6-teredo", r"\[2001:0:[^\]]*\]"),
    ("v6-teredo-compressed", r"\[2001::[^\]]*\]"),
    ("v6-multicast", r"\[ff[0-9a-f][0-9a-f]:[^\]]*\]"),
    ("metadata", r"metadata\.google\.internal"),
];

/// Blocked only when the scan origin is not itself a loopback dev server.
const LOOPBACK_HOSTS: &[(&str, &str)] = &[
    ("loopback", r"127\.[0-9]+\.[0-9]+\.[0-9]+"),
    ("v6-loopback", r"\[(0*:)+0*1\]"),
    ("localhost", r"localhost"),
    ("localhost-subdomain", r"[a-z0-9.-]+\.localhost"),
];

impl PrivateNetworkRules {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn host_patterns(self) -> impl Iterator<Item = &'static (&'static str, &'static str)> {
        let loopback: &'static [(&'static str, &'static str)] = if self.allow_local_dev {
            &[]
        } else {
            LOOPBACK_HOSTS
        };
        ALWAYS_BLOCKED_HOSTS.iter().chain(loopback.iter())
    }

    /// One `url-filter` per host pattern: any scheme, optional userinfo, the
    /// host, an optional trailing dot, then the port, path, query, or fragment
    /// separator that always follows a host in the canonical URL WebKit
    /// matches against. WHATWG parsing keeps the trailing dot on a domain host
    /// (`http://localhost./x`), and `network_policy` trims it before deciding,
    /// so the filters must tolerate it or macOS would admit what every other
    /// layer refuses.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn url_filters(self) -> Vec<String> {
        self.host_patterns()
            .map(|(_, host)| format!(r"^[a-z][a-z0-9+.-]*://([^/?#@]*@)?({host})\.?[:/?#]"))
            .collect()
    }

    /// The identifier this mode's rule document is compiled under.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn rules_identifier(self) -> &'static str {
        if self.allow_local_dev {
            PRIVATE_NETWORK_RULES_IDENTIFIER_LOCAL_DEV
        } else {
            PRIVATE_NETWORK_RULES_IDENTIFIER_STRICT
        }
    }

    /// The WKContentRuleList document WebKit compiles.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn to_webkit_json(self) -> String {
        let rules: Vec<serde_json::Value> = self
            .url_filters()
            .into_iter()
            .map(|filter| {
                serde_json::json!({
                    "trigger": {
                        "url-filter": filter,
                        "url-filter-is-case-sensitive": false
                    },
                    "action": { "type": "block" }
                })
            })
            .collect();
        serde_json::Value::Array(rules).to_string()
    }

    /// The per-request decision the Windows filter applies: the same
    /// nonblocking policy the redirect and subresource checks use. Only
    /// network schemes are judged; `data:`, `blob:`, and `about:` carry no
    /// network request and all parse. A URI that does not parse at all is
    /// blocked: the filter cannot judge what it cannot read, and failing open
    /// there would hand the page an unvetted request.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn blocks(self, raw_url: &str) -> bool {
        let Ok(mut url) = url::Url::parse(raw_url) else {
            return true;
        };
        match url.scheme() {
            "http" | "https" => {}
            "ws" => {
                let _ = url.set_scheme("http");
            }
            "wss" => {
                let _ = url.set_scheme("https");
            }
            _ => return false,
        }
        crate::network_policy::validate_page_subresource_target(&url, self.allow_local_dev).is_err()
    }
}

/// Completion signal from the platform installer to `analyze_url`. The macOS
/// completion block is a `Fn`, so the one-shot sender lives behind a mutex.
#[derive(Clone)]
pub(crate) struct RulesReady(Arc<Mutex<Option<tokio::sync::oneshot::Sender<bool>>>>);

impl RulesReady {
    pub(crate) fn new(sender: tokio::sync::oneshot::Sender<bool>) -> Self {
        Self(Arc::new(Mutex::new(Some(sender))))
    }

    pub(crate) fn signal(&self, installed: bool) {
        if let Some(sender) = self.0.lock().ok().and_then(|mut slot| slot.take()) {
            let _ = sender.send(installed);
        }
    }
}

/// macOS: compile the rules into a WKContentRuleList and attach it to the
/// webview's WKUserContentController. Compilation is asynchronous, so the
/// page must stay on `about:blank` until `ready` fires.
#[cfg(target_os = "macos")]
pub(crate) fn install_private_network_rules(
    webview: &tauri::WebviewWindow,
    rules: PrivateNetworkRules,
    ready: RulesReady,
) -> Result<(), String> {
    let encoded_rules = rules.to_webkit_json();
    let rules_identifier = rules.rules_identifier();
    webview
        .with_webview(move |platform| {
            use objc2::rc::Retained;
            use objc2::MainThreadMarker;
            use objc2_foundation::{NSError, NSString};
            use objc2_web_kit::{
                WKContentRuleList, WKContentRuleListStore, WKUserContentController,
            };

            let Some(main_thread) = MainThreadMarker::new() else {
                ready.signal(false);
                return;
            };
            // SAFETY: `controller()` is the WKUserContentController that owns
            // this webview's content rules, and with_webview runs on the main
            // thread that owns it; retaining keeps it alive for the block.
            let controller: Option<Retained<WKUserContentController>> = unsafe {
                Retained::retain(platform.controller().cast::<WKUserContentController>())
            };
            let Some(controller) = controller else {
                ready.signal(false);
                return;
            };
            // SAFETY: the default store is a main-thread singleton.
            let Some(store) = (unsafe { WKContentRuleListStore::defaultStore(main_thread) }) else {
                ready.signal(false);
                return;
            };
            let identifier = NSString::from_str(rules_identifier);
            let encoded = NSString::from_str(&encoded_rules);
            let completion =
                block2::RcBlock::new(move |list: *mut WKContentRuleList, _error: *mut NSError| {
                    let installed = !list.is_null();
                    if installed {
                        // SAFETY: WebKit passes a live rule list on success.
                        unsafe { controller.addContentRuleList(&*list) };
                        tracing::info!("Analyzer private-network rules installed");
                    } else {
                        tracing::warn!("Analyzer private-network rules failed to compile");
                    }
                    ready.signal(installed);
                });
            // SAFETY: WebKit copies both strings before returning.
            unsafe {
                store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
                    Some(&identifier),
                    Some(&encoded),
                    Some(&completion),
                );
            }
        })
        .map_err(|error| error.to_string())
}

/// Windows: answer every private-network request with 403 from a
/// WebResourceRequested handler. The filter is installed synchronously, so
/// `ready` fires before the closure returns.
#[cfg(windows)]
pub(crate) fn install_private_network_rules(
    webview: &tauri::WebviewWindow,
    rules: PrivateNetworkRules,
    ready: RulesReady,
) -> Result<(), String> {
    webview
        .with_webview(move |platform| {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                ICoreWebView2, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            };
            use webview2_com::{take_pwstr, WebResourceRequestedEventHandler};
            use windows::core::{w, PWSTR};
            use windows::Win32::System::Com::IStream;

            let environment = platform.environment();
            let outcome: windows::core::Result<()> = (|| {
                let core: ICoreWebView2 = unsafe { platform.controller().CoreWebView2()? };
                unsafe {
                    core.AddWebResourceRequestedFilter(
                        w!("*"),
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    )?;
                }
                let handler =
                    WebResourceRequestedEventHandler::create(Box::new(move |_webview, args| {
                        let Some(args) = args else {
                            return Ok(());
                        };
                        let request = unsafe { args.Request()? };
                        let mut uri = PWSTR::null();
                        unsafe { request.Uri(&mut uri)? };
                        let target = take_pwstr(uri);
                        if rules.blocks(&target) {
                            let response = unsafe {
                                environment.CreateWebResourceResponse(
                                    None::<&IStream>,
                                    403,
                                    w!("Forbidden"),
                                    w!(""),
                                )?
                            };
                            unsafe { args.SetResponse(&response)? };
                        }
                        Ok(())
                    }));
                // WebView2 event registration tokens are plain i64 handles.
                let mut token = 0i64;
                unsafe { core.add_WebResourceRequested(&handler, &mut token)? };
                Ok(())
            })();
            if outcome.is_ok() {
                tracing::info!("Analyzer private-network rules installed");
            } else {
                tracing::warn!("Analyzer private-network rules failed to install");
            }
            ready.signal(outcome.is_ok());
        })
        .map_err(|error| error.to_string())
}

/// Linux: webkit2gtk-rs 2.0 does not bind `WebKitUserContentFilterStore`
/// (the `webkit2gtk-sys` crate exposes `webkit_user_content_filter_store_new`
/// and `webkit_user_content_manager_add_filter` for a later raw-FFI task).
/// Until then the top-level navigation gate is the only subresource control
/// on Linux, and the privacy copy says so.
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) fn install_private_network_rules(
    _webview: &tauri::WebviewWindow,
    _rules: PrivateNetworkRules,
    ready: RulesReady,
) -> Result<(), String> {
    tracing::info!("Analyzer private-network subresource rules are unavailable on this platform");
    ready.signal(true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::PrivateNetworkRules;

    fn webkit_blocks(rules: PrivateNetworkRules, url: &str) -> bool {
        rules.url_filters().iter().any(|filter| {
            regex::RegexBuilder::new(filter)
                .case_insensitive(true)
                .build()
                .expect(filter)
                .is_match(url)
        })
    }

    const BOTH_MODES: [PrivateNetworkRules; 2] = [
        PrivateNetworkRules {
            allow_local_dev: false,
        },
        PrivateNetworkRules {
            allow_local_dev: true,
        },
    ];

    /// One representative literal per branch of
    /// `network_policy::is_private_or_internal_ip`, so a branch that gains no
    /// rule pattern fails the parity tests instead of passing unnoticed. The
    /// two loopback branches live in `LOOPBACK_TARGETS` because they are the
    /// only ones the local-dev mode may admit.
    const PRIVATE_TARGETS: &[&str] = &[
        // IPv4 is_private
        "http://10.0.0.5/admin",
        "https://172.16.0.1:8443/x.css",
        "http://172.20.0.1/",
        "http://172.31.0.1/",
        "http://192.168.1.1/reboot",
        // IPv4 is_link_local
        "http://169.254.169.254/latest/meta-data/",
        // IPv4 is_unspecified, and the rest of 0.0.0.0/8
        "http://0.0.0.0:8080/",
        "http://0.1.2.3/",
        // IPv4 is_broadcast
        "http://255.255.255.255/",
        // IPv4 carrier-grade NAT
        "http://100.64.0.1/",
        "http://100.127.0.1/",
        // IPv4 IETF protocol assignments
        "http://192.0.0.9/",
        // IPv4 benchmarking
        "http://198.18.0.1/",
        // IPv4 is_multicast
        "http://224.0.0.1/",
        "http://239.255.255.250/",
        // IPv4 reserved 240.0.0.0/4
        "http://240.0.0.1/",
        // IPv6 unique local
        "http://[fc00::1]/",
        // IPv6 unicast link local
        "http://[fe80::1]/",
        // IPv6 site local (fec0::/10)
        "http://[fec0::1]/",
        "http://[feff:ffff::1]/",
        // IPv6 is_multicast
        "http://[ff02::1]/",
        // IPv6 is_unspecified
        "http://[::]/",
        // IPv6 embedded IPv4-mapped, in both spellings
        "http://[::ffff:10.0.0.1]/",
        "http://[::ffff:a00:1]/",
        // IPv6 embedded IPv4-compatible, private and loopback
        "http://[::a00:1]/",
        "http://[::7f00:1]/",
        // IPv6 NAT64 well-known prefix
        "http://[64:ff9b::a00:1]/",
        // IPv6 NAT64 local-use prefix
        "http://[64:ff9b:1::808:808]/",
        // IPv6 6to4
        "http://[2002:a00:1::]/",
        // IPv6 Teredo server address, then the inverted client address
        "http://[2001:0:a00:1::]/",
        "http://[2001:0:808:808::f5ff:fffe]/",
        // Cloud metadata name, bare and with the trailing dot WHATWG keeps
        "http://metadata.google.internal/computeMetadata/v1/",
        "http://metadata.google.internal./v1",
        // Non-http schemes and userinfo reach the same decision
        "ws://10.0.0.5:9000/socket",
        "http://user:pass@10.0.0.5/",
    ];

    /// The `is_loopback` branches for both address families. Public scans
    /// refuse them; a scan whose own origin is loopback keeps them.
    const LOOPBACK_TARGETS: &[&str] = &[
        "http://127.0.0.1:5173/app.css",
        "http://localhost:3000/api",
        "http://app.localhost:3000/",
        "http://[::1]:3000/",
        // WHATWG keeps the trailing dot on a domain host, so these reach the
        // same loopback services under a spelling the filters must also see.
        "http://localhost./x",
        "http://localhost.:3000/x",
        "http://app.localhost./x",
    ];

    const PUBLIC_TARGETS: &[&str] = &[
        "https://cdn.example.com/style.css",
        "https://fonts.example.org/f.css",
        "https://1.1.1.1/",
        "https://223.255.255.255/",
        "https://198.17.0.1/",
        "https://192.0.1.1/",
        "https://[2606:4700::1111]/",
        "https://[2001:4860:4860::8888]/",
        "https://localhost.run/",
        "https://127.0.0.1.example.com/",
    ];

    /// WebKit's rule compiler accepts a strict subset of what the `regex`
    /// crate parses, so a pattern the tests happily evaluate can still fail
    /// the whole rule list at run time and take every macOS scan down with
    /// it. Alternation, counted repetition, non-capturing groups, and any
    /// escape outside the three used here are the ones that bite.
    #[test]
    fn every_filter_stays_inside_the_webkit_dialect() {
        for rules in BOTH_MODES {
            for filter in rules.url_filters() {
                assert!(filter.starts_with('^'), "unanchored: {filter}");
                for unsupported in ["|", "{", "(?"] {
                    assert!(!filter.contains(unsupported), "{unsupported} in {filter}");
                }
                let bytes = filter.as_bytes();
                for (index, byte) in bytes.iter().enumerate() {
                    if *byte != b'\\' {
                        continue;
                    }
                    let escaped = bytes.get(index + 1).copied().unwrap_or(b' ');
                    assert!(
                        matches!(escaped, b'.' | b'[' | b']'),
                        "unsupported escape {} in {filter}",
                        escaped as char
                    );
                }
            }
        }
    }

    #[test]
    fn webkit_rules_block_everything_the_policy_refuses() {
        for rules in BOTH_MODES {
            for url in PRIVATE_TARGETS.iter().chain(LOOPBACK_TARGETS) {
                if rules.blocks(url) {
                    assert!(
                        webkit_blocks(rules, url),
                        "{url} (allow_local_dev={})",
                        rules.allow_local_dev
                    );
                }
            }
        }
    }

    #[test]
    fn private_targets_are_blocked_under_both_modes() {
        for rules in BOTH_MODES {
            for url in PRIVATE_TARGETS {
                assert!(rules.blocks(url), "policy: {url}");
                assert!(webkit_blocks(rules, url), "webkit: {url}");
            }
        }
    }

    #[test]
    fn loopback_is_blocked_for_public_scans_and_allowed_for_local_dev() {
        let [public, local] = BOTH_MODES;
        for url in LOOPBACK_TARGETS {
            assert!(public.blocks(url) && webkit_blocks(public, url), "{url}");
            assert!(!local.blocks(url) && !webkit_blocks(local, url), "{url}");
        }
    }

    #[test]
    fn public_targets_and_non_network_schemes_pass() {
        for rules in BOTH_MODES {
            for url in PUBLIC_TARGETS {
                assert!(!rules.blocks(url), "policy: {url}");
                assert!(!webkit_blocks(rules, url), "webkit: {url}");
            }
            for url in [
                "about:blank",
                "data:text/plain,hi",
                "blob:https://example.com/abc",
            ] {
                assert!(!rules.blocks(url), "{url}");
            }
        }
    }

    // A trailing dot survives WHATWG parsing on a domain host, so before this
    // the macOS content rules admitted `http://localhost./x` while
    // `network_policy` (which trims the dot) refused it.
    #[test]
    fn a_trailing_dot_does_not_slip_a_host_past_the_content_rules() {
        let [public, local] = BOTH_MODES;
        for url in [
            "http://localhost./x",
            "http://localhost.:3000/x",
            "http://app.localhost./x",
        ] {
            assert!(webkit_blocks(public, url), "public: {url}");
            assert!(!webkit_blocks(local, url), "local dev: {url}");
        }
        for rules in BOTH_MODES {
            for url in [
                "http://metadata.google.internal./v1",
                "http://10.0.0.5./x",
                "http://169.254.169.254./latest/",
            ] {
                assert!(rules.blocks(url), "policy: {url}");
                assert!(webkit_blocks(rules, url), "webkit: {url}");
            }
            // A public host that merely starts with a blocked name still passes.
            for url in ["https://localhost.run/", "https://127.0.0.1.example.com/"] {
                assert!(!rules.blocks(url), "policy: {url}");
                assert!(!webkit_blocks(rules, url), "webkit: {url}");
            }
        }
    }

    // The Windows filter cannot judge a URI it cannot read, and failing open
    // there would hand the scanned page an unvetted request.
    #[test]
    fn an_unparseable_uri_is_blocked_under_both_modes() {
        for rules in BOTH_MODES {
            for raw in ["", "http://", "://nowhere", "http://[::1", "not a url"] {
                assert!(rules.blocks(raw), "{raw:?} must fail closed");
            }
        }
    }

    // Two analyzer webviews can compile concurrently; one identifier for both
    // documents would have each overwrite the other's rules.
    #[test]
    fn each_mode_compiles_under_its_own_identifier() {
        let [public, local] = BOTH_MODES;
        assert_ne!(public.rules_identifier(), local.rules_identifier());
        assert!(public.rules_identifier().ends_with("-strict"));
        assert!(local.rules_identifier().ends_with("-local-dev"));
    }

    #[test]
    fn rule_document_is_valid_webkit_json() {
        let public: serde_json::Value = serde_json::from_str(
            &PrivateNetworkRules {
                allow_local_dev: false,
            }
            .to_webkit_json(),
        )
        .expect("rule document parses");
        let rules = public.as_array().expect("array");
        assert_eq!(
            rules.len(),
            super::ALWAYS_BLOCKED_HOSTS.len() + super::LOOPBACK_HOSTS.len()
        );
        for rule in rules {
            assert_eq!(rule["action"]["type"], "block");
            assert_eq!(rule["trigger"]["url-filter-is-case-sensitive"], false);
            assert!(rule["trigger"]["url-filter"]
                .as_str()
                .expect("url-filter")
                .starts_with("^[a-z]"));
        }
        let local: serde_json::Value = serde_json::from_str(
            &PrivateNetworkRules {
                allow_local_dev: true,
            }
            .to_webkit_json(),
        )
        .expect("rule document parses");
        assert_eq!(
            local.as_array().expect("array").len(),
            super::ALWAYS_BLOCKED_HOSTS.len()
        );
    }

    #[tokio::test]
    async fn rules_ready_fires_once() {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let ready = super::RulesReady::new(sender);
        ready.signal(true);
        ready.signal(false);
        assert_eq!(receiver.await, Ok(true));
    }
}
