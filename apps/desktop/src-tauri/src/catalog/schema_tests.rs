//! Adversarial limits for signed catalog packs.

use super::*;

fn guide(steps: usize) -> GuideEntry {
    GuideEntry {
        effort: Effort::Quick,
        effort_minutes: 5,
        default: (0..steps).map(|i| format!("step {i}")).collect(),
        frameworks: None,
    }
}

fn pack_with(guides: Vec<(&str, GuideEntry)>) -> CatalogPack {
    CatalogPack {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        catalog_version: "2026.07.26".into(),
        release_sequence: 1,
        published_at: "2026-07-26T00:00:00Z".into(),
        minimum_engine_version: "1.0.0".into(),
        guides: guides
            .into_iter()
            .map(|(id, g)| (id.to_string(), g))
            .collect(),
    }
}

#[test]
fn accepts_a_well_formed_pack() {
    assert!(pack_with(vec![("security.csp", guide(3))])
        .validate()
        .is_ok());
}

#[test]
fn rejects_an_unknown_schema_version() {
    let mut pack = pack_with(vec![("security.csp", guide(1))]);
    pack.schema_version = SUPPORTED_SCHEMA_VERSION + 1;
    assert!(matches!(
        pack.validate(),
        Err(SchemaError::UnsupportedSchemaVersion { .. })
    ));
}

#[test]
fn rejects_unknown_fields_rather_than_ignoring_them() {
    let json = r#"{
        "schema_version": 1,
        "catalog_version": "2026.07.26",
        "release_sequence": 1,
        "published_at": "2026-07-26T00:00:00Z",
        "minimum_engine_version": "1.0.0",
        "guides": {},
        "on_activate": "curl https://example.test"
    }"#;
    let parsed: Result<CatalogPack, _> = serde_json::from_str(json);
    assert!(parsed.is_err(), "unknown top-level field must not parse");
}

#[test]
fn rejects_an_unknown_field_inside_a_guide() {
    let json = r#"{
        "schema_version": 1,
        "catalog_version": "2026.07.26",
        "release_sequence": 1,
        "published_at": "2026-07-26T00:00:00Z",
        "minimum_engine_version": "1.0.0",
        "guides": {
            "security.csp": {
                "effort": "quick",
                "effort_minutes": 5,
                "default": ["a"],
                "run_command": "rm -rf /"
            }
        }
    }"#;
    let parsed: Result<CatalogPack, _> = serde_json::from_str(json);
    assert!(parsed.is_err(), "unknown guide field must not parse");
}

#[test]
fn rejects_an_effort_value_outside_the_closed_set() {
    let json = r#"{"effort":"trivial","effort_minutes":1,"default":["a"]}"#;
    let parsed: Result<GuideEntry, _> = serde_json::from_str(json);
    assert!(parsed.is_err(), "effort is a closed set");
}

#[test]
fn rejects_too_many_entries() {
    let guides = (0..crate::constants::CATALOG_MAX_ENTRIES + 1)
        .map(|i| (format!("check.{i}"), guide(1)))
        .collect();
    let pack = CatalogPack {
        guides,
        ..pack_with(vec![])
    };
    assert!(matches!(
        pack.validate(),
        Err(SchemaError::TooManyEntries { .. })
    ));
}

#[test]
fn rejects_too_many_steps() {
    let pack = pack_with(vec![(
        "security.csp",
        guide(crate::constants::CATALOG_MAX_STEPS_PER_GUIDE + 1),
    )]);
    assert!(matches!(
        pack.validate(),
        Err(SchemaError::TooManySteps { .. })
    ));
}

#[test]
fn rejects_an_oversized_step() {
    let mut entry = guide(1);
    entry.default = vec!["x".repeat(crate::constants::CATALOG_MAX_STEP_CHARS + 1)];
    let pack = pack_with(vec![("security.csp", entry)]);
    assert!(matches!(
        pack.validate(),
        Err(SchemaError::StepTooLong { .. })
    ));
}

#[test]
fn counts_step_length_in_characters_not_bytes() {
    // A step at the limit made of multi-byte characters is legitimate content,
    // not an attack, and must not be refused for its byte length.
    let mut entry = guide(1);
    entry.default = vec!["é".repeat(crate::constants::CATALOG_MAX_STEP_CHARS)];
    let pack = pack_with(vec![("security.csp", entry)]);
    assert!(pack.validate().is_ok());
}

#[test]
fn rejects_too_many_framework_variants() {
    let mut entry = guide(1);
    entry.frameworks = Some(
        (0..crate::constants::CATALOG_MAX_FRAMEWORK_VARIANTS + 1)
            .map(|i| (format!("framework{i}"), vec!["step".to_string()]))
            .collect(),
    );
    let pack = pack_with(vec![("security.csp", entry)]);
    assert!(matches!(
        pack.validate(),
        Err(SchemaError::TooManyFrameworks { .. })
    ));
}

#[test]
fn rejects_an_overlong_check_id() {
    let long = "x".repeat(crate::constants::CATALOG_MAX_KEY_CHARS + 1);
    let pack = pack_with(vec![(long.as_str(), guide(1))]);
    assert!(matches!(
        pack.validate(),
        Err(SchemaError::KeyTooLong { .. })
    ));
}

#[test]
fn rejects_a_guide_with_no_steps() {
    let pack = pack_with(vec![("security.csp", guide(0))]);
    assert!(matches!(
        pack.validate(),
        Err(SchemaError::EmptyGuide { .. })
    ));
}

#[test]
fn enforces_step_limits_inside_framework_variants_too() {
    // The default steps are the obvious path; a variant is the one an
    // implementation is likely to forget to bound.
    let mut entry = guide(1);
    entry.frameworks = Some(
        [(
            "next".to_string(),
            vec!["x".repeat(crate::constants::CATALOG_MAX_STEP_CHARS + 1)],
        )]
        .into_iter()
        .collect(),
    );
    let pack = pack_with(vec![("security.csp", entry)]);
    assert!(matches!(
        pack.validate(),
        Err(SchemaError::StepTooLong { .. })
    ));
}
