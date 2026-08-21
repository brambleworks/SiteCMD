//! Best-effort projection of completed scans into verified-good baselines.

use crate::core::scanner::ScanResult;
use crate::db::Database;
use sitecmd_engine::profile::{FieldValue, Observation, RouteSet};
use std::collections::BTreeMap;

fn merge_site_observations(observations: &[Observation], scan_id: Option<i64>) -> Observation {
    let mut values = BTreeMap::new();
    for observation in observations {
        for value in &observation.values {
            let field = value.field();
            match (values.get(&field), value) {
                (
                    Some(FieldValue::ThirdPartyOrigins(current)),
                    FieldValue::ThirdPartyOrigins(next),
                ) => {
                    values.insert(field, FieldValue::ThirdPartyOrigins(current.union(next)));
                }
                (None, _) => {
                    values.insert(field, value.clone());
                }
                _ => {}
            }
        }
    }
    Observation {
        values: values.into_values().collect(),
        scan_id,
    }
}

/// Record what this scan observed about the site.
pub(crate) fn record_baseline_observation(
    db: &Database,
    site_id: i64,
    scan_id: Option<i64>,
    result: &ScanResult,
) {
    let Some(facts) = result.site_facts.as_ref() else {
        return;
    };
    record_baseline_observations(db, site_id, scan_id, std::slice::from_ref(facts));
}

/// Record one complete multi-page session as one site observation. Page-level
/// facts are merged before comparison so page order cannot open or clear a
/// site-level drift record.
pub(crate) fn record_baseline_observations(
    db: &Database,
    site_id: i64,
    scan_id: Option<i64>,
    facts: &[Observation],
) {
    let mut observation = merge_site_observations(facts, scan_id);
    if let Some(routes) = known_routes(db, site_id) {
        observation.push(FieldValue::RouteSet(routes));
    }
    if observation.is_empty() {
        return;
    }
    let observed_at = chrono::Utc::now();
    if let Err(error) = db.apply_verified_good_observation(site_id, observation, observed_at) {
        tracing::warn!("could not record the site baseline observation: {error}");
    }
}

/// Return known canonical routes, or `None` when discovery has not run.
fn known_routes(db: &Database, site_id: i64) -> Option<RouteSet> {
    let pages = db.get_pages(site_id).ok()?;
    if pages.is_empty() {
        return None;
    }
    Some(RouteSet::new(pages.into_iter().map(|page| {
        sitecmd_engine::route::canonical_path(&page.path)
    })))
}

#[cfg(test)]
mod tests {
    use sitecmd_engine::profile::{FieldValue, Observation, OriginSet};

    fn origins(values: &[&str]) -> Observation {
        Observation {
            values: vec![FieldValue::ThirdPartyOrigins(OriginSet::from_origins(
                values.iter().map(|value| (*value).to_string()),
            ))],
            scan_id: None,
        }
    }

    #[test]
    fn a_session_merges_page_origins_before_comparing_the_site_baseline() {
        let merged = super::merge_site_observations(
            &[
                origins(&["https://cdn-a.test"]),
                origins(&["https://cdn-b.test"]),
            ],
            Some(91),
        );

        let set = merged
            .values
            .iter()
            .find_map(|value| match value {
                FieldValue::ThirdPartyOrigins(set) => Some(set),
                _ => None,
            })
            .expect("merged origins");
        assert_eq!(
            set.origins.values,
            ["https://cdn-a.test", "https://cdn-b.test"]
        );
        assert_eq!(merged.scan_id, Some(91));
    }
}
