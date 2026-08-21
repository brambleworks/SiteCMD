//! Integration suggestions and optional cache-backed issue enrichments.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

use crate::core::types_work_items::Enrichment;
use crate::integrations::IntegrationType;

/// Function pointer type for per-(check_id, integration) enrichment readers.
/// Reads from `integration_enrichment_cache`; returns None when data is absent
/// or stale (>5 min). No API calls -- cheap read-path only.
pub type EnricherFn = fn(
    check_id: &str,
    cache: &crate::core::correlation::enrichments::EnrichmentCache,
) -> Result<Option<Enrichment>, String>;

pub struct IntegrationHint {
    pub check_id: &'static str,
    pub integration: IntegrationType,
    pub value_prop: &'static str,
    /// Optional enrichment reader. Set when a structured data signal exists
    /// in `integration_enrichment_cache` for this (check_id, integration) pair.
    /// Per-integration writers are follow-up work; None means suggestion-only.
    pub enricher: Option<EnricherFn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct IntegrationSuggestion {
    pub check_id: String,
    pub integration: IntegrationType,
    pub value_prop: String,
}

pub const INTEGRATION_HINTS: &[IntegrationHint] = &[
    // Performance
    IntegrationHint {
        check_id: "performance.lcp",
        integration: IntegrationType::GoogleSearchConsole,
        value_prop: "See real-user LCP (Chrome UX Report field data) for URLs Google has crawled.",
        enricher: Some(crate::core::correlation::enrichments::gsc_field_lcp),
    },
    IntegrationHint {
        check_id: "performance.lcp",
        integration: IntegrationType::Plausible,
        value_prop: "See which pages are hit hardest by slow LCP.",
        enricher: None,
    },
    IntegrationHint {
        check_id: "performance.lcp",
        integration: IntegrationType::UptimeRobot,
        value_prop: "Track real-user LCP over time and get alerted when it regresses.",
        enricher: None,
    },
    IntegrationHint {
        check_id: "performance.cls",
        integration: IntegrationType::GoogleSearchConsole,
        value_prop: "See real-user CLS field data from Chrome UX Report for crawled URLs.",
        enricher: Some(crate::core::correlation::enrichments::gsc_field_cls),
    },
    IntegrationHint {
        check_id: "performance.inp",
        integration: IntegrationType::GoogleSearchConsole,
        value_prop: "See real-user INP field data from Chrome UX Report for crawled URLs.",
        enricher: Some(crate::core::correlation::enrichments::gsc_field_inp),
    },
    IntegrationHint {
        check_id: "performance.cache_headers",
        integration: IntegrationType::Cloudflare,
        value_prop: "See cache hit rates and which assets are bypassing the edge.",
        enricher: Some(crate::core::correlation::enrichments::cf_cache_hit_rate),
    },
    IntegrationHint {
        check_id: "performance.ttfb",
        integration: IntegrationType::UptimeRobot,
        value_prop: "Track TTFB over time and get alerted when it spikes.",
        enricher: Some(crate::core::correlation::enrichments::uptime_ttfb_history),
    },
    IntegrationHint {
        check_id: "performance.compression",
        integration: IntegrationType::Cloudflare,
        value_prop: "Cloudflare can compress responses at the edge without server changes.",
        enricher: None,
    },
    // SEO
    IntegrationHint {
        check_id: "seo.indexing.not-indexed",
        integration: IntegrationType::GoogleSearchConsole,
        value_prop: "See exactly which pages Google is skipping and why.",
        enricher: Some(crate::core::correlation::enrichments::gsc_search_impressions_drop),
    },
    IntegrationHint {
        check_id: "seo.indexing.crawl-error",
        integration: IntegrationType::GoogleSearchConsole,
        value_prop: "See crawl error counts and trends directly from Google Search Console.",
        enricher: Some(crate::core::correlation::enrichments::gsc_recent_crawl_errors),
    },
    IntegrationHint {
        check_id: "seo.robots.blocked",
        integration: IntegrationType::GoogleSearchConsole,
        value_prop: "Confirm Google is honoring your robots rules correctly.",
        enricher: None,
    },
    IntegrationHint {
        check_id: "seo.canonical.mismatch",
        integration: IntegrationType::GoogleSearchConsole,
        value_prop: "See which canonical Google chose and why.",
        enricher: None,
    },
    // Infrastructure
    IntegrationHint {
        check_id: "infrastructure.uptime",
        integration: IntegrationType::UptimeRobot,
        value_prop: "Detect outages in minutes instead of on your next scan.",
        enricher: Some(crate::core::correlation::enrichments::uptime_recent_downtime),
    },
    IntegrationHint {
        check_id: "infrastructure.ssl-expiring",
        integration: IntegrationType::UptimeRobot,
        value_prop: "Get paged if the cert causes downtime before you can rotate.",
        enricher: Some(crate::core::correlation::enrichments::uptime_cert_expires_in),
    },
    IntegrationHint {
        check_id: "infrastructure.ssl-mismatch",
        integration: IntegrationType::UptimeRobot,
        value_prop: "Verify cert chain details and get alerted on mismatch-caused outages.",
        enricher: Some(crate::core::correlation::enrichments::uptime_cert_chain),
    },
    IntegrationHint {
        check_id: "infrastructure.ci-failure",
        integration: IntegrationType::GitHub,
        value_prop: "See which workflow is failing and jump to the run logs.",
        enricher: None,
    },
    IntegrationHint {
        check_id: "infrastructure.server-errors",
        integration: IntegrationType::Cloudflare,
        value_prop: "See 5xx breakdown by origin and worker at the edge.",
        enricher: Some(crate::core::correlation::enrichments::cf_recent_five_xx_spike),
    },
    IntegrationHint {
        check_id: "infrastructure.origin-error",
        integration: IntegrationType::Cloudflare,
        value_prop: "Track origin error rates and distinguish Cloudflare vs origin failures.",
        enricher: Some(crate::core::correlation::enrichments::cf_recent_origin_errors),
    },
    // Security
    IntegrationHint {
        check_id: "security.bot-traffic",
        integration: IntegrationType::Cloudflare,
        value_prop: "See bot scoring and block traffic at the edge.",
        enricher: Some(crate::core::correlation::enrichments::cf_bot_traffic_pct),
    },
    IntegrationHint {
        check_id: "security.csp",
        integration: IntegrationType::GitHub,
        value_prop: "Commit the header config straight from your repo with a PR template.",
        enricher: None,
    },
    // Analytics
    IntegrationHint {
        check_id: "analytics.traffic-drop",
        integration: IntegrationType::Plausible,
        value_prop: "See which page or source is responsible for the drop.",
        enricher: Some(crate::core::correlation::enrichments::plausible_top_falling_page),
    },
    IntegrationHint {
        check_id: "analytics.conversion-drop",
        integration: IntegrationType::Plausible,
        value_prop: "See where in the funnel users are falling off.",
        enricher: Some(crate::core::correlation::enrichments::plausible_top_falling_funnel),
    },
];

#[tracing::instrument(skip(connected, dismissed), fields(check_id = %check_id))]
pub fn resolve_integration_suggestions(
    check_id: &str,
    connected: &HashSet<IntegrationType>,
    dismissed: &HashSet<(String, IntegrationType)>,
) -> Vec<IntegrationSuggestion> {
    INTEGRATION_HINTS
        .iter()
        .filter(|h| h.check_id == check_id)
        .filter(|h| !connected.contains(&h.integration))
        .filter(|h| !dismissed.contains(&(h.check_id.to_string(), h.integration.clone())))
        .take(2)
        .map(|h| IntegrationSuggestion {
            check_id: h.check_id.to_string(),
            integration: h.integration.clone(),
            value_prop: h.value_prop.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::IntegrationType;
    use std::collections::HashSet;

    #[test]
    fn suggestion_excluded_if_integration_connected() {
        let connected: HashSet<IntegrationType> =
            [IntegrationType::GoogleSearchConsole].into_iter().collect();
        let dismissed: HashSet<(String, IntegrationType)> = HashSet::new();
        let out = resolve_integration_suggestions("performance.lcp", &connected, &dismissed);
        assert!(out
            .iter()
            .all(|s| s.integration != IntegrationType::GoogleSearchConsole));
    }

    #[test]
    fn suggestion_excluded_if_pair_dismissed() {
        let connected: HashSet<IntegrationType> = HashSet::new();
        let mut dismissed: HashSet<(String, IntegrationType)> = HashSet::new();
        dismissed.insert((
            "performance.lcp".to_string(),
            IntegrationType::GoogleSearchConsole,
        ));
        let out = resolve_integration_suggestions("performance.lcp", &connected, &dismissed);
        assert!(out
            .iter()
            .all(|s| s.integration != IntegrationType::GoogleSearchConsole));
    }

    #[test]
    fn max_two_suggestions_per_check_id() {
        let connected = HashSet::new();
        let dismissed = HashSet::new();
        // performance.lcp has 3 raw hints; the cap must limit output to 2.
        let out = resolve_integration_suggestions("performance.lcp", &connected, &dismissed);
        assert_eq!(
            out.len(),
            2,
            "the .take(2) cap must apply when >2 hints exist for a check_id"
        );
    }

    #[test]
    fn every_hint_has_non_empty_value_prop() {
        for h in INTEGRATION_HINTS {
            assert!(
                !h.value_prop.is_empty(),
                "empty value_prop on {}",
                h.check_id
            );
            assert!(!h.check_id.is_empty(), "empty check_id");
        }
    }

    #[test]
    fn enricher_wired_on_expected_hint_pairs() {
        // Each expected pair must have an enricher.
        let expected_with_enricher = [
            ("performance.lcp", IntegrationType::GoogleSearchConsole),
            ("performance.cls", IntegrationType::GoogleSearchConsole),
            ("performance.inp", IntegrationType::GoogleSearchConsole),
            (
                "seo.indexing.not-indexed",
                IntegrationType::GoogleSearchConsole,
            ),
            (
                "seo.indexing.crawl-error",
                IntegrationType::GoogleSearchConsole,
            ),
            ("infrastructure.uptime", IntegrationType::UptimeRobot),
            ("infrastructure.ssl-expiring", IntegrationType::UptimeRobot),
            ("infrastructure.ssl-mismatch", IntegrationType::UptimeRobot),
            ("performance.ttfb", IntegrationType::UptimeRobot),
            ("security.bot-traffic", IntegrationType::Cloudflare),
            ("performance.cache_headers", IntegrationType::Cloudflare),
            ("infrastructure.server-errors", IntegrationType::Cloudflare),
            ("infrastructure.origin-error", IntegrationType::Cloudflare),
            ("analytics.traffic-drop", IntegrationType::Plausible),
            ("analytics.conversion-drop", IntegrationType::Plausible),
        ];

        for (check_id, integration) in &expected_with_enricher {
            let hint = INTEGRATION_HINTS
                .iter()
                .find(|h| h.check_id == *check_id && h.integration == *integration);
            assert!(
                hint.is_some(),
                "missing hint for ({}, {:?})",
                check_id,
                integration
            );
            assert!(
                hint.unwrap().enricher.is_some(),
                "enricher should be Some for ({}, {:?})",
                check_id,
                integration
            );
        }
    }
}
