use super::*;

use crate::core::code_scan::canonical_code_check_id;
use crate::core::code_scan::registry::CODE_CHECKS;

#[test]
fn the_inventory_knows_every_code_scan_check_this_build_can_emit() {
    for check in CODE_CHECKS {
        let check_id = canonical_code_check_id(check.slug);
        assert!(
            CURRENT_INVENTORY.lookup(&check_id).is_some(),
            "{check_id} is emittable but absent from the inventory, so a release that added it would read as a deploy regression"
        );
    }
}

#[test]
fn the_inventory_knows_every_manifest_check_with_its_contract() {
    let manifest = sitecmd_engine::manifest::capability_manifest();
    for entry in &manifest.entries {
        let recorded = CURRENT_INVENTORY
            .lookup(&entry.check)
            .unwrap_or_else(|| panic!("{} missing from the inventory", entry.check));
        assert_eq!(recorded.contract.as_deref(), Some(entry.contract.as_str()));
    }
}

#[test]
fn code_check_ids_are_recorded_in_their_canonical_lifecycle_form() {
    // Blame and the lifecycle compare canonical ids. An inventory keyed by raw
    // slugs would answer "unattested" for every code finding.
    let check_id = canonical_code_check_id(CODE_CHECKS[0].slug);
    assert!(
        check_id.starts_with("code_scan.") || check_id.contains('.'),
        "unexpected canonical form: {check_id}"
    );
    assert!(CURRENT_INVENTORY.lookup(&check_id).is_some());
}

#[test]
fn a_code_check_carries_no_contract_because_nothing_versions_its_meaning() {
    let check_id = canonical_code_check_id(CODE_CHECKS[0].slug);
    let entry = CURRENT_INVENTORY
        .lookup(&check_id)
        .expect("code check recorded");
    assert!(
        entry.contract.is_none(),
        "a contract here would claim a promise the code registry does not make"
    );
}

#[test]
fn a_web_run_states_the_transport_facts_a_verdict_can_depend_on() {
    let profile = execution_profile(ObservedSurface::Web, Some("health"), false, None);
    assert_eq!(profile.transport.as_deref(), Some("reqwest_rustls"));
    assert_eq!(profile.tls_client.as_deref(), Some("rustls"));
    assert_eq!(profile.trust_authority.as_deref(), Some("webpki_roots"));
    assert_eq!(profile.resolver.as_deref(), Some("system"));
    assert_eq!(profile.scan_profile.as_deref(), Some("health"));
}

#[test]
fn a_run_without_a_browser_states_no_browser_facts() {
    let profile = execution_profile(ObservedSurface::Web, Some("health"), false, None);
    assert!(profile.browser_engine.is_none());
    assert!(profile.axe_version.is_none());
    assert_eq!(profile.layers_run, vec![LAYER_TRANSPORT.to_string()]);
}

#[test]
fn a_run_with_a_browser_names_the_engine_and_the_axe_version() {
    let profile = execution_profile(ObservedSurface::Web, Some("health"), true, Some("621.1.15"));
    assert_eq!(profile.browser_engine.as_deref(), Some(browser_engine()));
    assert_eq!(profile.browser_build.as_deref(), Some("621.1.15"));
    assert_eq!(
        profile.axe_version.as_deref(),
        Some(sitecmd_engine::browser::payload::AXE_CORE_VERSION)
    );
    assert_eq!(
        profile.layers_run,
        vec![LAYER_TRANSPORT.to_string(), LAYER_BROWSER.to_string()]
    );
}

#[test]
fn the_browser_epoch_stays_unstated_because_no_corpus_certifies_it_here() {
    let profile = execution_profile(ObservedSurface::Web, None, true, None);
    assert!(profile.browser_epoch.is_none());
}

#[test]
fn a_code_run_claims_no_network_facts_at_all() {
    let profile = execution_profile(ObservedSurface::Code, Some("code"), false, None);
    assert!(profile.transport.is_none());
    assert!(profile.tls_client.is_none());
    assert!(profile.trust_authority.is_none());
    assert!(profile.resolver.is_none());
    assert!(profile.browser_engine.is_none());
    assert_eq!(profile.layers_run, vec![LAYER_CODE.to_string()]);
}

#[test]
fn the_stamp_names_this_crate_version_and_the_manifest_it_carries() {
    let stamped = stamp(ObservedSurface::Web, Some("health"), false, None);
    assert_eq!(stamped.engine_release, env!("CARGO_PKG_VERSION"));
    assert_eq!(
        stamped.manifest_digest,
        sitecmd_engine::manifest::capability_manifest().manifest_digest
    );
}
