use super::*;

#[test]
fn the_default_row_is_the_shape_most_checks_have() {
    let entry = Entry::new("seo.thin_content");
    assert_eq!(entry.lane, HostedLane::Artifact);
    assert_eq!(entry.class, CheckClass::Deterministic);
    assert_eq!(entry.scope, CheckScope::Page);
    assert_eq!(entry.resolved_requires(), vec![RuntimeFact::PageArtifact]);
    // A pure document check compares on its contract alone: the same bytes
    // through the same portable code cannot disagree between runtimes.
    assert!(entry.resolved_compare_on().is_empty());
}

#[test]
fn a_browser_row_compares_within_an_engine_and_an_epoch() {
    let entry = Entry::new("performance.lcp")
        .browser()
        .measurement(MeasurementUnit::Milliseconds);
    assert_eq!(
        entry.resolved_compare_on(),
        vec![
            CompareDimension::BrowserEngine,
            CompareDimension::BrowserEpoch
        ]
    );
    assert_eq!(entry.resolved_requires(), vec![RuntimeFact::Browser]);
    assert_eq!(entry.measurement_unit, Some(MeasurementUnit::Milliseconds));
}

#[test]
fn declared_dimensions_and_needs_win_over_the_lane_defaults() {
    let entry = Entry::new("security.dns.spf")
        .probe()
        .origin()
        .needs(&[RuntimeFact::Resolver])
        .compare_on(&[CompareDimension::TrustAuthority]);
    assert_eq!(entry.resolved_requires(), vec![RuntimeFact::Resolver]);
    assert_eq!(
        entry.resolved_compare_on(),
        vec![CompareDimension::TrustAuthority]
    );
}

#[test]
fn an_unsupported_row_needs_nothing_and_compares_on_nothing() {
    let entry = Entry::new("seo.title").unsupported();
    assert!(entry.resolved_requires().is_empty());
    assert!(entry.resolved_compare_on().is_empty());
}

#[test]
fn the_contract_moves_with_the_revision_and_nothing_else() {
    let base = Entry::new("security.headers.csp");
    let same = Entry::new("security.headers.csp")
        .probe()
        .origin()
        .session();
    assert_eq!(base.contract(), same.contract());
    assert_ne!(base.contract(), base.revision(2).contract());
    assert_ne!(
        base.contract(),
        Entry::new("security.headers.hsts").contract()
    );
}

#[test]
fn a_measurement_unit_is_part_of_the_contract() {
    let milliseconds = Entry::new("performance.lcp").measurement(MeasurementUnit::Milliseconds);
    let ratio = Entry::new("performance.lcp").measurement(MeasurementUnit::Ratio);
    assert_ne!(milliseconds.contract(), ratio.contract());
}

#[test]
fn every_measurement_has_exactly_one_unit_and_other_checks_have_none() {
    for entry in capability_manifest().entries {
        assert_eq!(
            entry.measurement_unit.is_some(),
            entry.class == CheckClass::Measurement,
            "{} has an inconsistent class/unit declaration",
            entry.check
        );
    }
}

#[test]
fn external_facts_fold_into_the_contract() {
    let pinned = Entry::new("accessibility.axe.")
        .family()
        .contract_extra(&["4.11.2"]);
    let upgraded = Entry::new("accessibility.axe.")
        .family()
        .contract_extra(&["4.12.0"]);
    assert_ne!(pinned.contract(), upgraded.contract());
}

#[test]
fn a_contract_is_a_short_stable_hex_string() {
    let contract = Entry::new("security.dns.spf").contract();
    assert_eq!(contract.len(), 16, "{contract}");
    assert!(
        contract.chars().all(|c| c.is_ascii_hexdigit()),
        "{contract}"
    );
    assert_eq!(contract, "e57c98162a303632");
}

#[test]
fn a_dynamic_id_resolves_to_its_family() {
    let manifest = capability_manifest();
    let axe = manifest
        .entry("accessibility.axe.color-contrast")
        .expect("axe rule resolves");
    assert_eq!(axe.check, "accessibility.axe.");
    assert!(axe.family);

    let exposed = manifest
        .entry("security.exposed_files.wpconfigphp")
        .expect("probed path resolves");
    assert_eq!(exposed.check, "security.exposed_files.");
}

#[test]
fn an_exact_entry_wins_over_a_family_it_sits_under() {
    let manifest = capability_manifest();
    let summary = manifest
        .entry("security.exposed_files.summary")
        .expect("summary resolves");
    assert!(!summary.family);
    assert_eq!(summary.check, "security.exposed_files.summary");
}

#[test]
fn an_unregistered_id_resolves_to_nothing() {
    // The gate at connect quarantines what it cannot resolve rather than
    // guessing, so returning None here is the whole mechanism.
    assert!(capability_manifest().entry("security.invented").is_none());
}

#[test]
fn the_document_digest_is_a_function_of_content_and_not_of_formatting() {
    let manifest = capability_manifest();
    assert_eq!(manifest.digest().len(), 16);
    assert_eq!(manifest.digest(), capability_manifest().digest());

    let pretty = serde_json::to_string_pretty(&manifest).expect("serializes");
    let round_tripped: CapabilityManifest = serde_json::from_str(&pretty).expect("parses");
    assert_eq!(round_tripped, manifest);
    assert_eq!(
        document_digest(round_tripped.schema_version, &round_tripped.entries),
        manifest.manifest_digest
    );
}

#[test]
fn the_digest_moves_when_any_entry_moves() {
    let manifest = capability_manifest();
    let mut mutated = manifest.entries.clone();
    mutated[0].class = match mutated[0].class {
        CheckClass::Deterministic => CheckClass::Measurement,
        _ => CheckClass::Deterministic,
    };
    assert_ne!(
        document_digest(SCHEMA_VERSION, &mutated),
        manifest.manifest_digest
    );
}
