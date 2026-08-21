//! Cross-page aggregation: distinct page URLs an IssueGroup affects.
//!
//! Data is already in `group.instances[].page_url` from the v2 work-item
//! pipeline; this just flattens, dedupes, and sorts for stable rendering.

use crate::core::types_work_items::IssueGroup;

pub fn resolve_affected_pages(group: &IssueGroup) -> Vec<String> {
    let mut pages: Vec<String> = group
        .instances
        .iter()
        .filter_map(|i| i.page_url.clone())
        .collect();
    pages.sort();
    pages.dedup();
    pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Severity;
    use crate::core::types_work_items::{IssueInstance, IssueStatus};

    fn mk_instance(page: Option<&str>) -> IssueInstance {
        IssueInstance {
            id: 0,
            source: "web_scan".into(),
            signal_id: "sig".into(),
            producer_check_id: None,
            url: None,
            page_url: page.map(String::from),
            severity: Severity::High,
            title: "t".into(),
            description: "d".into(),
            category: None,
            check_status: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            detail_json: None,
            first_seen_at: 0,
            last_seen_at: 0,
            confidence: None,
            confidence_reason: None,
            domain: None,
            relative_path: None,
            line: None,
            producer_fix_prompt: None,
            producer_category: None,
        }
    }

    fn mk_group(pages: Vec<Option<&str>>) -> IssueGroup {
        IssueGroup {
            check_id: "x".into(),
            category: "x".into(),
            severity: Severity::High,
            title: "t".into(),
            description: "d".into(),
            instances: pages.into_iter().map(mk_instance).collect(),
            sources: vec!["web_scan".into()],
            status: IssueStatus::New,
            snooze_until: None,
            block_reason: None,
            impact_score: 0.0,
            likely_causes: Vec::new(),
            suggested_integrations: Vec::new(),
            fix_locations: Vec::new(),
            transitive_causes: Vec::new(),
            downstream_effects: Vec::new(),
            recent_events: Vec::new(),
            enrichments: Vec::new(),
            correlation_evidence: Vec::new(),
            affected_pages: Vec::new(),
            cross_env_signal: None,
            cross_project_pattern: None,
            display_confidence: None,
            observation_count: 0,
            anomaly_score: None,
        }
    }

    #[test]
    fn dedupes_and_sorts_pages() {
        let g = mk_group(vec![Some("/pricing"), Some("/checkout"), Some("/pricing")]);
        let pages = resolve_affected_pages(&g);
        assert_eq!(pages, vec!["/checkout", "/pricing"]);
    }

    #[test]
    fn ignores_null_page_urls() {
        let g = mk_group(vec![Some("/pricing"), None, Some("/about")]);
        let pages = resolve_affected_pages(&g);
        assert_eq!(pages, vec!["/about", "/pricing"]);
    }

    #[test]
    fn empty_when_no_pages() {
        let g = mk_group(vec![None, None]);
        let pages = resolve_affected_pages(&g);
        assert!(pages.is_empty());
    }
}
