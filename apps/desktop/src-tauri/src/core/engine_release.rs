//! Builds release stamps from version, manifest, analyzer, and check inventory
//! facts compiled into the desktop binary.

use std::sync::LazyLock;

use sitecmd_engine::browser::payload::AXE_CORE_VERSION;
use sitecmd_engine::release::{CheckInventory, ExecutionProfile, ReleaseStamp};

/// The release every observation this binary produces is stamped with.
pub const ENGINE_RELEASE: &str = env!("CARGO_PKG_VERSION");

/// The HTTP client profile behind every fetch: reqwest over rustls.
const TRANSPORT_PROFILE: &str = "reqwest_rustls";
/// The TLS client profile behind every handshake, including the sync
/// certificate probe.
const TLS_CLIENT_PROFILE: &str = "rustls";
/// Chain validation runs against the operating system's certificate store
/// through rustls-platform-verifier, the same verifier reqwest itself uses.
const TRUST_AUTHORITY: &str = "platform_verifier";
/// DNS answers come from the system resolver configuration.
const RESOLVER: &str = "system";

/// Layer names for `layers_run`. A layer that did not run cannot be read as
/// absence of what it would have found.
pub const LAYER_TRANSPORT: &str = "transport";
pub const LAYER_BROWSER: &str = "browser";
pub const LAYER_CODE: &str = "code";

/// Browser engine used for accessibility and vitals. Findings from different
/// engines are not assumed comparable.
pub fn browser_engine() -> &'static str {
    if cfg!(feature = "browser") {
        "chromium"
    } else if cfg!(target_os = "macos") {
        "webkit"
    } else if cfg!(target_os = "windows") {
        "webview2"
    } else {
        "webkitgtk"
    }
}

/// Which surface produced the observation. Web and code runs execute different
/// layers and state different runtime facts, and a profile that claimed a TLS
/// client for a filesystem scan would be fiction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedSurface {
    Web,
    Code,
}

/// The runtime facts for one run.
pub fn execution_profile(
    surface: ObservedSurface,
    scan_profile: Option<&str>,
    browser_ran: bool,
    browser_build: Option<&str>,
) -> ExecutionProfile {
    match surface {
        ObservedSurface::Web => {
            let mut layers_run = vec![LAYER_TRANSPORT.to_string()];
            if browser_ran {
                layers_run.push(LAYER_BROWSER.to_string());
            }
            ExecutionProfile {
                browser_engine: browser_ran.then(|| browser_engine().to_string()),
                browser_build: browser_ran
                    .then(|| browser_build.map(str::to_string))
                    .flatten(),
                // Unstated on purpose: the compatibility epoch is certified
                // against a corpus the hosted runner owns, and a locally
                // invented one would be a fact with no authority behind it.
                browser_epoch: None,
                axe_version: browser_ran.then(|| AXE_CORE_VERSION.to_string()),
                resolver: Some(RESOLVER.to_string()),
                transport: Some(TRANSPORT_PROFILE.to_string()),
                tls_client: Some(TLS_CLIENT_PROFILE.to_string()),
                trust_authority: Some(TRUST_AUTHORITY.to_string()),
                scan_profile: scan_profile.map(str::to_string),
                layers_run,
            }
        }
        ObservedSurface::Code => ExecutionProfile {
            scan_profile: scan_profile.map(str::to_string),
            layers_run: vec![LAYER_CODE.to_string()],
            ..Default::default()
        },
    }
}

/// The stamp for one run.
pub fn stamp(
    surface: ObservedSurface,
    scan_profile: Option<&str>,
    browser_ran: bool,
    browser_build: Option<&str>,
) -> ReleaseStamp {
    ReleaseStamp::current(
        ENGINE_RELEASE,
        execution_profile(surface, scan_profile, browser_ran, browser_build),
    )
}

/// Complete build inventory: contracted Web checks and unversioned Code checks.
pub static CURRENT_INVENTORY: LazyLock<CheckInventory> = LazyLock::new(|| {
    CheckInventory::from_manifest(&sitecmd_engine::manifest::capability_manifest())
        .with_unversioned(crate::core::code_scan::registry::registered_code_check_ids())
});

#[cfg(test)]
#[path = "engine_release_tests.rs"]
mod tests;
