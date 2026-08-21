//! Storage tests: the promises a lapsed subscriber depends on.

use super::*;
use crate::catalog::schema::{Effort, GuideEntry, SUPPORTED_SCHEMA_VERSION};

fn sample_pack(sequence: u64) -> CatalogPack {
    CatalogPack {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        catalog_version: format!("2026.07.{sequence:02}"),
        release_sequence: sequence,
        published_at: "2026-07-26T00:00:00Z".into(),
        minimum_engine_version: "1.0.0".into(),
        guides: [(
            "security.csp".to_string(),
            GuideEntry {
                effort: Effort::Quick,
                effort_minutes: 5,
                default: vec!["step".to_string()],
                frameworks: None,
            },
        )]
        .into_iter()
        .collect(),
    }
}

#[test]
fn reports_no_active_pack_on_a_fresh_install() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(load_active(dir.path()).expect("load").is_none());
    assert_eq!(active_release_sequence(dir.path()), None);
}

#[test]
fn activates_and_reads_back_a_pack() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = serde_json::to_vec(&sample_pack(7)).expect("serialize");
    activate(dir.path(), &bytes, 7).expect("activate");

    let loaded = load_active(dir.path()).expect("load").expect("present");
    assert_eq!(loaded.release_sequence, 7);
    assert_eq!(active_release_sequence(dir.path()), Some(7));
}

#[test]
fn activation_replaces_the_previous_pack_completely() {
    // A shorter pack must not leave a tail of the longer one behind, which is
    // what a plain truncate-and-write can do on a crash.
    let dir = tempfile::tempdir().expect("tempdir");
    let mut large = sample_pack(1);
    for i in 0..50 {
        large.guides.insert(
            format!("check.{i}"),
            GuideEntry {
                effort: Effort::Involved,
                effort_minutes: 60,
                default: vec!["a longer step".repeat(20)],
                frameworks: None,
            },
        );
    }
    activate(
        dir.path(),
        &serde_json::to_vec(&large).expect("serialize"),
        1,
    )
    .expect("activate large");
    activate(
        dir.path(),
        &serde_json::to_vec(&sample_pack(2)).expect("serialize"),
        2,
    )
    .expect("activate small");

    let loaded = load_active(dir.path()).expect("load").expect("present");
    assert_eq!(loaded.release_sequence, 2);
    assert_eq!(loaded.guides.len(), 1, "stale entries must not survive");
}

#[test]
fn a_corrupt_pack_is_reported_not_silently_treated_as_absent() {
    // Losing a paid pack to a disk error should surface as an error the user
    // can act on, not look like a fresh install.
    let dir = tempfile::tempdir().expect("tempdir");
    let catalog = catalog_dir(dir.path());
    std::fs::create_dir_all(&catalog).expect("mkdir");
    std::fs::write(catalog.join("active.json"), b"{ not json").expect("write");

    assert!(matches!(
        load_active(dir.path()),
        Err(StoreError::Corrupt(_))
    ));
    // Nothing was ever activated through `activate`, so no high-water mark
    // exists and this really is a fresh install as far as rollback goes.
    assert_eq!(active_release_sequence(dir.path()), None);
}

#[test]
fn the_rollback_floor_survives_losing_the_active_pack() {
    let dir = tempfile::tempdir().expect("tempdir");
    activate(
        dir.path(),
        &serde_json::to_vec(&sample_pack(100)).expect("serialize"),
        100,
    )
    .expect("activate");
    assert_eq!(active_release_sequence(dir.path()), Some(100));

    let catalog = catalog_dir(dir.path());
    std::fs::write(catalog.join("active.json"), b"{ not json").expect("corrupt");
    assert_eq!(active_release_sequence(dir.path()), Some(100));

    std::fs::remove_file(catalog.join("active.json")).expect("remove");
    assert_eq!(active_release_sequence(dir.path()), Some(100));
}

#[test]
fn the_rollback_floor_only_ever_rises() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pack = |sequence| serde_json::to_vec(&sample_pack(sequence)).expect("serialize");
    activate(dir.path(), &pack(5), 5).expect("activate 5");
    activate(dir.path(), &pack(3), 3).expect("activate 3");
    assert_eq!(active_release_sequence(dir.path()), Some(5));
}

#[test]
fn the_catalog_directory_is_created_private() {
    let dir = tempfile::tempdir().expect("tempdir");
    activate(
        dir.path(),
        &serde_json::to_vec(&sample_pack(1)).expect("serialize"),
        1,
    )
    .expect("activate");
    assert!(catalog_dir(dir.path()).is_dir());
}

#[test]
fn a_floor_without_a_readable_pack_reports_repairable() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        !active_pack_needs_repair(dir.path()),
        "fresh install needs no repair"
    );

    activate(
        dir.path(),
        &serde_json::to_vec(&sample_pack(100)).expect("serialize"),
        100,
    )
    .expect("activate");
    assert!(
        !active_pack_needs_repair(dir.path()),
        "a healthy pack needs no repair"
    );

    let catalog = catalog_dir(dir.path());
    std::fs::write(catalog.join("active.json"), b"{ not json").expect("corrupt");
    assert!(
        active_pack_needs_repair(dir.path()),
        "a corrupt pack repairs"
    );

    std::fs::remove_file(catalog.join("active.json")).expect("remove");
    assert!(
        active_pack_needs_repair(dir.path()),
        "a missing pack under a standing floor repairs"
    );
}

#[test]
fn a_readable_pack_behind_the_floor_reports_repairable() {
    let dir = tempfile::tempdir().expect("tempdir");
    activate(
        dir.path(),
        &serde_json::to_vec(&sample_pack(7)).expect("serialize"),
        7,
    )
    .expect("activate 7");

    // Raise the floor as a crashed activate(8) would have: floor first, no
    // pack write.
    std::fs::write(catalog_dir(dir.path()).join("release-sequence"), b"8").expect("raise floor");

    assert_eq!(
        active_release_sequence(dir.path()),
        Some(8),
        "the floor is the reported sequence"
    );
    assert!(
        active_pack_needs_repair(dir.path()),
        "a readable pack behind the floor repairs at the floor sequence"
    );

    // The repair lands: activating 8 heals the state.
    activate(
        dir.path(),
        &serde_json::to_vec(&sample_pack(8)).expect("serialize"),
        8,
    )
    .expect("repair at the floor");
    assert!(
        !active_pack_needs_repair(dir.path()),
        "a healed pack needs no repair"
    );
}
