//! Parity test for the generated capability manifest.
//!
//! Regenerate with `cargo test -p sitecmd-engine --test capability_manifest -- --ignored regenerate`.

use sitecmd_engine::manifest::{capability_manifest, CapabilityManifest, HostedLane};
use std::collections::BTreeSet;

const PUBLISHED: &str = include_str!("../manifest/capability_manifest.json");

const CORPORA: [(&str, &str); 3] = [
    (
        "golden.json",
        include_str!("../fixtures/checks/golden.json"),
    ),
    (
        "golden_probes.json",
        include_str!("../fixtures/checks/golden_probes.json"),
    ),
    (
        "golden_browser.json",
        include_str!("../fixtures/checks/golden_browser.json"),
    ),
];

fn published() -> CapabilityManifest {
    serde_json::from_str(PUBLISHED).expect("capability_manifest.json parses")
}

// Every check id the golden corpora actually report a row under, which is
// every id this crate is proven to emit.
fn corpus_result_ids() -> BTreeSet<(String, String)> {
    let mut ids = BTreeSet::new();
    for (corpus_name, source) in CORPORA {
        let corpus: serde_json::Value =
            serde_json::from_str(source).unwrap_or_else(|error| panic!("{corpus_name}: {error}"));
        let cases = corpus["cases"].as_array().expect("cases array");
        for case in cases {
            let Some(expected) = case["expected"].as_array() else {
                continue;
            };
            for row in expected {
                let id = row["checkId"].as_str().expect("row carries a checkId");
                ids.insert((id.to_string(), corpus_name.to_string()));
            }
        }
    }
    ids
}

#[test]
fn the_published_document_matches_the_registry() {
    assert_eq!(
        published(),
        capability_manifest(),
        "manifest/capability_manifest.json is stale; rerun the ignored `regenerate` test"
    );
}

#[test]
fn the_published_digest_is_the_one_the_engine_computes() {
    let published = published();
    assert_eq!(published.digest(), capability_manifest().digest());
    assert!(!published.digest().is_empty());
}

#[test]
fn every_id_the_corpus_reports_has_an_entry() {
    let manifest = capability_manifest();
    let missing: Vec<String> = corpus_result_ids()
        .into_iter()
        .filter(|(id, _)| manifest.entry(id).is_none())
        .map(|(id, corpus)| format!("{id} (from {corpus})"))
        .collect();
    assert!(
        missing.is_empty(),
        "these ids appear on corpus result rows with no manifest entry, so an observation carrying one would be unresolvable: {missing:?}"
    );
}

#[test]
fn no_id_the_corpus_reports_claims_to_be_unsupported() {
    let manifest = capability_manifest();
    let mislabeled: Vec<String> = corpus_result_ids()
        .into_iter()
        .filter(|(id, _)| {
            manifest
                .entry(id)
                .is_some_and(|entry| entry.hosted == HostedLane::Unsupported)
        })
        .map(|(id, corpus)| format!("{id} (from {corpus})"))
        .collect();
    assert!(mislabeled.is_empty(), "{mislabeled:?}");
}

#[test]
fn the_corpus_covers_a_meaningful_share_of_the_registry() {
    let manifest = capability_manifest();
    let covered: BTreeSet<String> = corpus_result_ids()
        .into_iter()
        .filter_map(|(id, _)| manifest.entry(&id).map(|entry| entry.check.clone()))
        .collect();
    println!(
        "capability manifest: {} entries, {} exercised by the golden corpora",
        manifest.entries.len(),
        covered.len()
    );
    assert!(
        covered.len() >= 55,
        "corpus coverage fell to {}",
        covered.len()
    );
}

#[test]
#[ignore = "regenerates the published manifest; run deliberately"]
fn regenerate() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/manifest/capability_manifest.json"
    );
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&capability_manifest()).expect("manifest serializes")
    );
    std::fs::write(path, rendered).expect("write capability_manifest.json");
    println!("regenerated {path}");
}
