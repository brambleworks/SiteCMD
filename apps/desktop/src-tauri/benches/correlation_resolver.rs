use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::HashSet;
use std::hint::black_box;

use app_lib::checks::Severity;
use app_lib::core::types_work_items::{IssueGroup, IssueInstance, IssueStatus};
use app_lib::integrations::IntegrationType;

fn mk_group(check_id: &str, severity: Severity) -> IssueGroup {
    IssueGroup {
        check_id: check_id.to_string(),
        category: "performance".to_string(),
        severity,
        title: format!("Issue {check_id}"),
        description: "synthetic bench issue".to_string(),
        instances: vec![IssueInstance {
            id: 1,
            source: "web_scan".to_string(),
            signal_id: format!("sig-{check_id}"),
            url: Some("https://example.com".to_string()),
            page_url: Some("https://example.com/pricing".to_string()),
            severity,
            title: "synthetic".to_string(),
            description: "synthetic".to_string(),
            detail_json: None,
            first_seen_at: 0,
            last_seen_at: 0,
            confidence: None,
            domain: None,
            relative_path: None,
            line: None,
            producer_check_id: None,
            category: None,
            check_status: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            confidence_reason: None,
            producer_fix_prompt: None,
            producer_category: None,
        }],
        sources: vec!["web_scan".to_string()],
        status: IssueStatus::New,
        snooze_until: None,
        block_reason: None,
        impact_score: 50.0,
        // v3 fields default-initialized; resolver populates them
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

/// Synthetic 100-group set drawing from canonical performance/SEO/infra check_ids so
/// the multi-hop walker has real graph traversal work to do.
fn build_100_groups() -> Vec<IssueGroup> {
    let canonical_ids = [
        "performance.compression",
        "performance.lcp",
        "performance.cls",
        "performance.inp",
        "performance.cache_headers",
        "performance.ttfb",
        "performance.render_blocking",
        "performance.unused_javascript",
        "performance.unused_css",
        "performance.modern_image_formats",
        "performance.page_weight",
        "performance.responsive_images",
        "performance.lazy_load_images",
        "seo.indexing.not-indexed",
        "seo.indexing.crawl-error",
        "seo.robots.blocked",
        "seo.canonical.missing",
        "seo.canonical.mismatch",
        "seo.mobile-viewport",
        "seo.sitemap.missing",
        "infrastructure.uptime",
        "infrastructure.ssl-expiring",
        "infrastructure.ssl-mismatch",
        "infrastructure.origin-error",
        "infrastructure.server-errors",
        "infrastructure.ci-failure",
        "security.https",
        "security.hsts",
        "security.csp",
        "security.mixed_content",
        "security.cors",
        "security.exposed-env",
        "security.bot-traffic",
        "dependencies.vulnerability",
        "dependencies.outdated-major",
        "analytics.traffic-drop",
        "analytics.conversion-drop",
    ];

    let mut groups = Vec::with_capacity(100);
    for (i, id) in canonical_ids.iter().cycle().take(100).enumerate() {
        let severity = match i % 4 {
            0 => Severity::Critical,
            1 => Severity::High,
            2 => Severity::Medium,
            _ => Severity::Low,
        };
        groups.push(mk_group(id, severity));
    }
    groups
}

fn bench_resolver_100_groups(c: &mut Criterion) {
    // Create a temporary database for the benchmark.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let db_path = dir.path().join("bench.db");
    let db = app_lib::db::Database::open(db_path).expect("failed to open database");

    let connected: HashSet<IntegrationType> = HashSet::new();
    let dismissed: HashSet<(String, IntegrationType)> = HashSet::new();

    c.bench_function("resolver_100_groups", |b| {
        b.iter(|| {
            let mut groups = build_100_groups();
            app_lib::core::correlation::resolver::enrich_issue_groups(
                &mut groups,
                1,
                "https://example.com",
                &db,
                &connected,
                &dismissed,
                None,
            )
            .unwrap();
            black_box(groups);
        });
    });
}

criterion_group!(benches, bench_resolver_100_groups);
criterion_main!(benches);
