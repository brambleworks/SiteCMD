//! Catalog-versus-baseline resolution order tests.

use super::*;
use crate::catalog::schema::{CatalogPack, Effort, GuideEntry, SUPPORTED_SCHEMA_VERSION};

fn pack_with(
    check_id: &str,
    steps: &[&str],
    framework_variant: Option<(&str, &str)>,
) -> CatalogPack {
    let mut entry = GuideEntry {
        effort: Effort::Quick,
        effort_minutes: 5,
        default: steps.iter().map(|s| s.to_string()).collect(),
        frameworks: None,
    };
    if let Some((framework, step)) = framework_variant {
        entry.frameworks = Some(
            [(framework.to_string(), vec![step.to_string()])]
                .into_iter()
                .collect(),
        );
    }
    CatalogPack {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        catalog_version: "2026.07.26".into(),
        release_sequence: 3,
        published_at: "2026-07-26T00:00:00Z".into(),
        minimum_engine_version: "1.0.0".into(),
        guides: [(check_id.to_string(), entry)].into_iter().collect(),
    }
}

fn activate(dir: &std::path::Path, pack: &CatalogPack) {
    let bytes = serde_json::to_vec(pack).expect("serialize");
    crate::catalog::store::activate(dir, &bytes, pack.release_sequence).expect("activate");
}

fn bundled(steps: &[&str]) -> Option<Vec<String>> {
    Some(steps.iter().map(|s| s.to_string()).collect())
}

#[test]
fn falls_back_to_bundled_guidance_when_no_catalog_is_active() {
    // Every build today takes this path: no endpoint is configured, so nothing
    // has ever activated. Behavior must be exactly what it was before.
    let dir = tempfile::tempdir().expect("tempdir");
    let resolved = resolve_guide(dir.path(), "security.csp", &[], bundled(&["bundled step"]))
        .expect("guidance");
    assert_eq!(resolved.source, GuideSource::Bundled);
    assert_eq!(resolved.steps, vec!["bundled step"]);
    assert!(resolved.catalog_version.is_none());
}

#[test]
fn the_catalog_wins_when_it_has_the_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    activate(
        dir.path(),
        &pack_with("security.csp", &["catalog step"], None),
    );

    let resolved = resolve_guide(dir.path(), "security.csp", &[], bundled(&["bundled step"]))
        .expect("guidance");
    assert_eq!(resolved.source, GuideSource::Catalog);
    assert_eq!(resolved.steps, vec!["catalog step"]);
    assert_eq!(resolved.catalog_version.as_deref(), Some("2026.07.26"));
    // Effort comes from the catalog entry, not a caller-invented default.
    assert_eq!(resolved.effort_minutes, Some(5));
}

#[test]
fn a_framework_variant_beats_the_catalog_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    activate(
        dir.path(),
        &pack_with(
            "security.csp",
            &["generic"],
            Some(("next", "next-specific")),
        ),
    );

    let resolved = resolve_guide(dir.path(), "security.csp", &["next"], None).expect("guidance");
    assert_eq!(resolved.steps, vec!["next-specific"]);

    // An unknown framework takes the default rather than returning nothing.
    let other = resolve_guide(dir.path(), "security.csp", &["svelte"], None).expect("guidance");
    assert_eq!(other.steps, vec!["generic"]);
}

#[test]
fn an_active_catalog_without_this_check_still_uses_bundled_guidance() {
    // The catalog enriches; it does not define the engine's coverage. A check
    // the catalog has nothing for must not lose its bundled guidance.
    let dir = tempfile::tempdir().expect("tempdir");
    activate(dir.path(), &pack_with("seo.title", &["catalog step"], None));

    let resolved = resolve_guide(dir.path(), "security.csp", &[], bundled(&["bundled step"]))
        .expect("guidance");
    assert_eq!(resolved.source, GuideSource::Bundled);
}

#[test]
fn returns_nothing_when_neither_source_has_the_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(resolve_guide(dir.path(), "security.csp", &[], None).is_none());
}

#[test]
fn a_corrupt_pack_falls_back_without_pretending_it_is_absent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let catalog_dir = crate::catalog::store::catalog_dir(dir.path());
    std::fs::create_dir_all(&catalog_dir).expect("mkdir");
    std::fs::write(catalog_dir.join("active.json"), b"{ not json").expect("write");

    let resolved = resolve_guide(dir.path(), "security.csp", &[], bundled(&["bundled step"]))
        .expect("guidance");
    assert_eq!(resolved.source, GuideSource::Bundled);

    // And the status command reports the error rather than "no catalog".
    assert!(matches!(
        crate::catalog::store::load_active(dir.path()),
        Err(crate::catalog::StoreError::Corrupt(_))
    ));
}

#[test]
fn a_dynamic_sub_id_resolves_to_its_parent_guide() {
    let dir = tempfile::tempdir().expect("tempdir");
    activate(
        dir.path(),
        &pack_with("security.cookies", &["parent guidance"], None),
    );

    let resolved =
        resolve_guide(dir.path(), "security.cookies.session", &[], None).expect("guidance");
    assert_eq!(resolved.source, GuideSource::Catalog);
    assert_eq!(resolved.steps, vec!["parent guidance"]);
}

#[test]
fn prefix_fallback_walks_all_the_way_up_like_the_bundled_lookup_does() {
    // Catalog and bundled lookup must share the same prefix fallback.
    let dir = tempfile::tempdir().expect("tempdir");
    activate(
        dir.path(),
        &pack_with("security", &["category guidance"], None),
    );

    let resolved = resolve_guide(dir.path(), "security.csp.header", &[], None).expect("guidance");
    assert_eq!(resolved.steps, vec!["category guidance"]);
}

#[test]
fn prefix_fallback_does_not_invent_a_match_for_an_unrelated_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    activate(
        dir.path(),
        &pack_with("security.cookies", &["cookies"], None),
    );

    assert!(resolve_guide(dir.path(), "seo.title", &[], None).is_none());
}

#[test]
fn variant_candidates_are_tried_most_specific_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut entry = GuideEntry {
        effort: Effort::Quick,
        effort_minutes: 5,
        default: vec!["generic".to_string()],
        frameworks: None,
    };
    entry.frameworks = Some(
        [
            ("next".to_string(), vec!["framework guidance".to_string()]),
            ("cloudflare".to_string(), vec!["cdn guidance".to_string()]),
        ]
        .into_iter()
        .collect(),
    );
    let pack = CatalogPack {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        catalog_version: "2026.07.26".into(),
        release_sequence: 1,
        published_at: "2026-07-26T00:00:00Z".into(),
        minimum_engine_version: "1.0.0".into(),
        guides: [("security.csp".to_string(), entry)].into_iter().collect(),
    };
    activate(dir.path(), &pack);

    let resolved =
        resolve_guide(dir.path(), "security.csp", &["next", "cloudflare"], None).expect("guidance");
    assert_eq!(resolved.steps, vec!["framework guidance"]);

    // With no framework detected, the CDN candidate still wins over default.
    let cdn_only =
        resolve_guide(dir.path(), "security.csp", &["cloudflare"], None).expect("guidance");
    assert_eq!(cdn_only.steps, vec!["cdn guidance"]);
}
