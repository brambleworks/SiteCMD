use super::*;
use crate::manifest::capability_manifest;

fn entry(contract: Option<&str>) -> InventoryEntry {
    InventoryEntry {
        contract: contract.map(str::to_string),
        compare_on: Vec::new(),
        family: false,
    }
}

fn family(contract: &str, compare_on: Vec<CompareDimension>) -> InventoryEntry {
    InventoryEntry {
        contract: Some(contract.to_string()),
        compare_on,
        family: true,
    }
}

fn stamp(execution: ExecutionProfile) -> ReleaseStamp {
    ReleaseStamp {
        engine_release: "1.5.4".into(),
        manifest_digest: "0123456789abcdef".into(),
        canonicalizer: CANONICALIZER_VERSION,
        crawl_profile: CRAWL_PROFILE,
        execution,
    }
}

fn basis<'a>(
    stamp: &'a ReleaseStamp,
    inventory: &'a CheckInventory,
) -> Option<ObservationBasis<'a>> {
    Some(ObservationBasis { stamp, inventory })
}

#[test]
fn a_check_only_the_later_build_could_produce_is_new_not_a_regression() {
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let old = CheckInventory::from_entries([("security.headers.csp", entry(Some("aaaa")))]);
    let new = CheckInventory::from_entries([
        ("security.headers.csp", entry(Some("aaaa"))),
        ("security.headers.coop", entry(Some("bbbb"))),
    ]);
    assert_eq!(
        comparability(
            "security.headers.coop",
            basis(&old_stamp, &old),
            basis(&new_stamp, &new)
        ),
        Comparability::NewCheck
    );
}

#[test]
fn a_check_the_later_build_dropped_is_retired_not_fixed() {
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let old = CheckInventory::from_entries([("seo.meta.keywords", entry(Some("aaaa")))]);
    let new = CheckInventory::default();
    assert_eq!(
        comparability(
            "seo.meta.keywords",
            basis(&old_stamp, &old),
            basis(&new_stamp, &new)
        ),
        Comparability::Retired
    );
}

#[test]
fn a_moved_contract_means_the_detector_changed_not_the_site() {
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let old = CheckInventory::from_entries([("security.ssl.expiry", entry(Some("aaaa")))]);
    let new = CheckInventory::from_entries([("security.ssl.expiry", entry(Some("bbbb")))]);
    assert_eq!(
        comparability(
            "security.ssl.expiry",
            basis(&old_stamp, &old),
            basis(&new_stamp, &new)
        ),
        Comparability::DetectorChanged
    );
}

#[test]
fn an_unchanged_contract_compares() {
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let inventory = CheckInventory::from_entries([("security.ssl.expiry", entry(Some("aaaa")))]);
    assert_eq!(
        comparability(
            "security.ssl.expiry",
            basis(&old_stamp, &inventory),
            basis(&new_stamp, &inventory)
        ),
        Comparability::Comparable
    );
}

#[test]
fn losing_a_contract_is_a_detector_change_because_the_promise_changed() {
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let old = CheckInventory::from_entries([("code.php.sql-injection", entry(Some("aaaa")))]);
    let new = CheckInventory::from_entries([("code.php.sql-injection", entry(None))]);
    assert_eq!(
        comparability(
            "code.php.sql-injection",
            basis(&old_stamp, &old),
            basis(&new_stamp, &new)
        ),
        Comparability::DetectorChanged
    );
}

#[test]
fn two_unversioned_checks_compare_on_existence_alone() {
    // Code checks are enumerable but carry no contract. Both builds have it,
    // so a difference in the finding is a difference in the code.
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let inventory = CheckInventory::default().with_unversioned(["hardcoded-secret"]);
    assert_eq!(
        comparability(
            "hardcoded-secret",
            basis(&old_stamp, &inventory),
            basis(&new_stamp, &inventory)
        ),
        Comparability::Comparable
    );
}

#[test]
fn a_missing_earlier_basis_concludes_nothing() {
    let new_stamp = stamp(ExecutionProfile::default());
    let inventory = CheckInventory::from_entries([("security.headers.csp", entry(Some("aaaa")))]);
    assert_eq!(
        comparability("security.headers.csp", None, basis(&new_stamp, &inventory)),
        Comparability::Unattested
    );
}

#[test]
fn an_id_neither_build_claims_is_unregistered_not_unattested() {
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let inventory = CheckInventory::from_entries([("security.headers.csp", entry(Some("aaaa")))]);
    assert_eq!(
        comparability(
            "cloudflare.5xx-rate-high",
            basis(&old_stamp, &inventory),
            basis(&new_stamp, &inventory)
        ),
        Comparability::Unregistered
    );
}

#[test]
fn a_dynamic_id_resolves_through_its_family_prefix() {
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let old = CheckInventory::from_entries([("accessibility.axe.", family("aaaa", vec![]))]);
    let new = CheckInventory::from_entries([("accessibility.axe.", family("bbbb", vec![]))]);
    assert_eq!(
        comparability(
            "accessibility.axe.color-contrast",
            basis(&old_stamp, &old),
            basis(&new_stamp, &new)
        ),
        Comparability::DetectorChanged
    );
}

#[test]
fn an_exact_entry_wins_over_a_family_that_covers_it() {
    let inventory = CheckInventory::from_entries([
        ("accessibility.axe.", family("aaaa", vec![])),
        ("accessibility.axe.label", entry(Some("cccc"))),
    ]);
    assert_eq!(
        inventory
            .lookup("accessibility.axe.label")
            .and_then(|found| found.contract.clone()),
        Some("cccc".to_string())
    );
}

#[test]
fn the_longest_matching_family_prefix_wins() {
    let inventory = CheckInventory::from_entries([
        ("code.", family("aaaa", vec![])),
        ("code.php.", family("bbbb", vec![])),
    ]);
    assert_eq!(
        inventory
            .lookup("code.php.eval")
            .and_then(|found| found.contract.clone()),
        Some("bbbb".to_string())
    );
}

#[test]
fn a_moved_dimension_the_check_declares_blocks_comparison() {
    let old_stamp = stamp(ExecutionProfile {
        browser_engine: Some("webkit".into()),
        ..Default::default()
    });
    let new_stamp = stamp(ExecutionProfile {
        browser_engine: Some("chromium".into()),
        ..Default::default()
    });
    let inventory = CheckInventory::from_entries([(
        "accessibility.axe.",
        family("aaaa", vec![CompareDimension::BrowserEngine]),
    )]);
    assert_eq!(
        comparability(
            "accessibility.axe.color-contrast",
            basis(&old_stamp, &inventory),
            basis(&new_stamp, &inventory)
        ),
        Comparability::ProfileChanged(CompareDimension::BrowserEngine)
    );
}

#[test]
fn a_moved_dimension_the_check_does_not_declare_is_irrelevant() {
    // A header check does not care which browser ran, and pretending it did
    // would sever comparability every time an unrelated fact moved.
    let old_stamp = stamp(ExecutionProfile {
        browser_engine: Some("webkit".into()),
        ..Default::default()
    });
    let new_stamp = stamp(ExecutionProfile {
        browser_engine: Some("chromium".into()),
        ..Default::default()
    });
    let inventory = CheckInventory::from_entries([("security.headers.csp", entry(Some("aaaa")))]);
    assert_eq!(
        comparability(
            "security.headers.csp",
            basis(&old_stamp, &inventory),
            basis(&new_stamp, &inventory)
        ),
        Comparability::Comparable
    );
}

#[test]
fn an_unstated_dimension_compares_equal_to_an_unstated_dimension() {
    // Both readings came from one installation, so the same unknown on both
    // sides is not evidence of a difference.
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile::default());
    let inventory = CheckInventory::from_entries([(
        "accessibility.axe.",
        family("aaaa", vec![CompareDimension::BrowserEpoch]),
    )]);
    assert_eq!(
        comparability(
            "accessibility.axe.label",
            basis(&old_stamp, &inventory),
            basis(&new_stamp, &inventory)
        ),
        Comparability::Comparable
    );
}

#[test]
fn stating_a_dimension_only_on_one_side_blocks_comparison() {
    let old_stamp = stamp(ExecutionProfile::default());
    let new_stamp = stamp(ExecutionProfile {
        axe_version: Some("4.10.2".into()),
        ..Default::default()
    });
    let inventory = CheckInventory::from_entries([(
        "accessibility.axe.",
        family("aaaa", vec![CompareDimension::AxeVersion]),
    )]);
    assert_eq!(
        comparability(
            "accessibility.axe.label",
            basis(&old_stamp, &inventory),
            basis(&new_stamp, &inventory)
        ),
        Comparability::ProfileChanged(CompareDimension::AxeVersion)
    );
}

#[test]
fn every_comparison_dimension_maps_to_a_profile_field() {
    // A dimension with no field would silently compare equal forever.
    let profile = ExecutionProfile {
        browser_engine: Some("chromium".into()),
        browser_build: Some("127.0.6533.88".into()),
        browser_epoch: Some("2026-07".into()),
        axe_version: Some("4.10.2".into()),
        resolver: Some("system".into()),
        transport: Some("reqwest_rustls".into()),
        tls_client: Some("rustls".into()),
        trust_authority: Some("webpki_roots".into()),
        scan_profile: Some("health".into()),
        layers_run: vec!["transport".into()],
    };
    for dimension in [
        CompareDimension::BrowserEngine,
        CompareDimension::BrowserEpoch,
        CompareDimension::AxeVersion,
        CompareDimension::TransportProfile,
        CompareDimension::TrustAuthority,
        CompareDimension::TlsClientProfile,
    ] {
        assert!(
            profile.dimension(dimension).is_some(),
            "{dimension:?} has no execution-profile field"
        );
    }
}

#[test]
fn the_inventory_carries_every_manifest_entry_with_its_contract() {
    let manifest = capability_manifest();
    let inventory = CheckInventory::from_manifest(&manifest);
    assert_eq!(inventory.len(), manifest.entries.len());
    for entry in &manifest.entries {
        let recorded = inventory
            .lookup(&entry.check)
            .expect("every manifest entry is in the inventory");
        assert_eq!(recorded.contract.as_deref(), Some(entry.contract.as_str()));
        assert_eq!(recorded.compare_on, entry.compare_on);
        assert_eq!(recorded.family, entry.family);
    }
}

#[test]
fn unversioned_ids_never_overwrite_a_manifest_contract() {
    let manifest = capability_manifest();
    let first = manifest.entries[0].check.clone();
    let inventory =
        CheckInventory::from_manifest(&manifest).with_unversioned([first.clone(), "code.x".into()]);
    assert!(inventory
        .lookup(&first)
        .and_then(|found| found.contract.clone())
        .is_some());
    assert!(inventory
        .lookup("code.x")
        .expect("unversioned id recorded")
        .contract
        .is_none());
}

#[test]
fn the_current_stamp_names_the_manifest_this_build_carries() {
    let stamp = ReleaseStamp::current("1.5.4", ExecutionProfile::default());
    assert_eq!(
        stamp.manifest_digest,
        capability_manifest().manifest_digest,
        "a stamp must never name a manifest the build does not hold"
    );
    assert_eq!(stamp.engine_release, "1.5.4");
    assert_eq!(stamp.canonicalizer, CANONICALIZER_VERSION);
    assert_eq!(stamp.crawl_profile, CRAWL_PROFILE);
}

#[test]
fn an_inventory_round_trips_through_its_stored_rows() {
    let manifest = capability_manifest();
    let inventory = CheckInventory::from_manifest(&manifest).with_unversioned(["hardcoded-secret"]);
    let stored: Vec<(String, InventoryEntry)> = inventory
        .iter()
        .map(|(check_id, entry)| (check_id.to_string(), entry.clone()))
        .collect();
    assert_eq!(CheckInventory::from_entries(stored), inventory);
}

#[test]
fn comparability_codes_are_distinct_and_stable() {
    let codes = [
        Comparability::Comparable.code(),
        Comparability::NewCheck.code(),
        Comparability::Retired.code(),
        Comparability::DetectorChanged.code(),
        Comparability::ProfileChanged(CompareDimension::AxeVersion).code(),
        Comparability::Unregistered.code(),
        Comparability::Unattested.code(),
    ];
    let unique: std::collections::BTreeSet<&str> = codes.iter().copied().collect();
    assert_eq!(unique.len(), codes.len());
    assert!(Comparability::Comparable.is_comparable());
    assert!(!Comparability::NewCheck.is_comparable());
    assert!(!Comparability::Unattested.is_comparable());
    assert!(
        !Comparability::Unregistered.is_comparable(),
        "an unregistered id is not a comparison the stamp made; callers decide what to do with it"
    );
}
