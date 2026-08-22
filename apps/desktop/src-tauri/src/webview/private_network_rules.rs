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

/// Identifier under which WebKit caches the compiled rule list. Bump the
/// suffix whenever the patterns change so a stale compiled list is not reused.
#[cfg(target_os = "macos")]
pub(crate) const PRIVATE_NETWORK_RULES_IDENTIFIER: &str = "sitecmd-private-network-v1";

#[derive(Debug, Clone, Copy)]
pub(crate) struct PrivateNetworkRules {
    /// A scan of an explicit loopback origin must still load its own assets.
    pub allow_local_dev: bool,
}

/// Host patterns in the regex subset WebKit's `url-filter` dialect and the
/// `regex` crate share: literals, character classes, groups, `*`, `+`, `?`,
/// and anchors. WebKit rejects alternation, so every numeric range is spelled
/// as its own pattern. Tests evaluate them with `regex` as the stand-in for
/// WebKit. The IPv6 transition prefixes are blocked whole because a regex
/// cannot decode the embedded IPv4 address; that is stricter than
/// `network_policy`, never looser.
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
    ("v6-mapped", r"\[::ffff:[0-9a-f.:]+\]"),
    ("v6-nat64", r"\[64:ff9b:[^\]]*\]"),
    ("v6-6to4", r"\[2002:[^\]]*\]"),
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
    /// host, then the port, path, query, or fragment separator that always
    /// follows a host in the canonical URL WebKit matches against.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn url_filters(self) -> Vec<String> {
        self.host_patterns()
            .map(|(_, host)| format!(r"^[a-z][a-z0-9+.-]*://([^/?#@]*@)?({host})[:/?#]"))
            .collect()
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
    /// network request.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn blocks(self, raw_url: &str) -> bool {
        let Ok(mut url) = url::Url::parse(raw_url) else {
            return false;
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
            let identifier = NSString::from_str(PRIVATE_NETWORK_RULES_IDENTIFIER);
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

    const PRIVATE_TARGETS: &[&str] = &[
        "http://10.0.0.5/admin",
        "https://172.16.0.1:8443/x.css",
        "http://192.168.1.1/reboot",
        "http://169.254.169.254/latest/meta-data/",
        "http://0.0.0.0:8080/",
        "http://100.64.0.1/",
        "http://192.0.0.9/",
        "http://198.18.0.1/",
        "http://224.0.0.1/",
        "http://240.0.0.1/",
        "http://[fc00::1]/",
        "http://[fe80::1]/",
        "http://[::ffff:10.0.0.1]/",
        "http://[64:ff9b::a00:1]/",
        "ws://10.0.0.5:9000/socket",
        "http://user:pass@10.0.0.5/",
        "http://metadata.google.internal/computeMetadata/v1/",
    ];

    const LOOPBACK_TARGETS: &[&str] = &[
        "http://127.0.0.1:5173/app.css",
        "http://localhost:3000/api",
        "http://app.localhost:3000/",
        "http://[::1]:3000/",
    ];

    const PUBLIC_TARGETS: &[&str] = &[
        "https://cdn.example.com/style.css",
        "https://fonts.example.org/f.css",
        "https://1.1.1.1/",
        "https://223.255.255.255/",
        "https://198.17.0.1/",
        "https://192.0.1.1/",
        "https://[2606:4700::1111]/",
        "https://localhost.run/",
        "https://127.0.0.1.example.com/",
    ];

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
