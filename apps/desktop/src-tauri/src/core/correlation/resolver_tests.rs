use super::*;
use crate::checks::Severity;
use crate::core::correlation::causal_graph::Confidence;
use crate::core::types_work_items::{Enrichment, Evidence, IssueGroup, IssueInstance};
use crate::db::test_helpers::{temp_db, temp_db_with_project};
use crate::db::work_items::WorkItemMetadata;
use std::sync::RwLock;

static ENV_VAR_LOCK: RwLock<()> = RwLock::new(());

fn make_group(check_id: &str) -> IssueGroup {
    IssueGroup {
        check_id: check_id.into(),
        category: "performance".into(),
        severity: Severity::Medium,
        title: "t".into(),
        description: "d".into(),
        instances: Vec::<IssueInstance>::new(),
        sources: vec!["web_scan".into()],
        status: crate::core::types_work_items::IssueStatus::New,
        snooze_until: None,
        block_reason: None,
        impact_score: 0.0,
        likely_causes: Vec::new(),
        suggested_integrations: Vec::new(),
        fix_locations: Vec::new(),
        transitive_causes: Vec::new(),
        downstream_effects: Vec::new(),
        recent_events: Vec::new(),
        enrichments: Vec::<Enrichment>::new(),
        correlation_evidence: Vec::<Evidence>::new(),
        affected_pages: Vec::new(),
        cross_env_signal: None,
        cross_project_pattern: None,
        display_confidence: None,
        observation_count: 0,
        anomaly_score: None,
    }
}

// Seed 10 co-resolved rows for (compression -> lcp) in a test DB.
fn seed_observations(db: &crate::db::Database, project_id: i64, count: u32) {
    db.execute(move |conn| {
        for i in 0..count {
            conn.execute(
                "INSERT INTO causal_link_observations
                 (project_id, cause_check_id, effect_check_id, observed_at, co_active, co_resolved, resolution_event_id)
                 VALUES (?1, 'performance.compression', 'performance.lcp', ?2, 1, 1, NULL)",
                rusqlite::params![project_id, i as i64 * 1000],
            ).unwrap();
        }
        Ok::<(), rusqlite::Error>(())
    })
    .unwrap()
    .unwrap();
}

#[test]
fn observation_history_raises_likely_cause_confidence() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    let db = temp_db_with_project();
    let project_id: i64 = 1;

    seed_observations(&db, project_id, 10);

    // Both compression and lcp are active.
    let mut groups = vec![
        make_group("performance.lcp"),
        make_group("performance.compression"),
    ];

    let connected = HashSet::new();
    let dismissed = HashSet::new();
    enrich_issue_groups(
        &mut groups,
        project_id,
        "https://example.com",
        &db,
        &connected,
        &dismissed,
        None,
    )
    .expect("enrich_issue_groups should not fail");

    let lcp_group = groups
        .iter()
        .find(|g| g.check_id == "performance.lcp")
        .expect("lcp group");

    // Should have compression as a likely cause.
    assert!(
        lcp_group
            .likely_causes
            .iter()
            .any(|c| c.check_id == "performance.compression"),
        "compression should be a likely cause of lcp"
    );

    // observation_count should be set (10 co-resolved rows).
    assert!(
        lcp_group.observation_count >= 10,
        "observation_count should be >= 10; got {}",
        lcp_group.observation_count
    );

    // display_confidence should be Some (we have at least one likely cause).
    assert!(
        lcp_group.display_confidence.is_some(),
        "display_confidence should be set when likely causes exist"
    );
}

#[test]
fn low_resolution_ratio_drops_cause_confidence() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    let db = temp_db_with_project();
    let project_id: i64 = 1;

    // Seed 1 co-resolved out of 10 total (ratio 0.1 < 0.2).
    // First insert 10 co_active rows, then only 1 co_resolved.
    db.execute(move |conn| {
        // 9 active-only (co_resolved=0)
        for i in 0..9 {
            conn.execute(
                "INSERT INTO causal_link_observations
                 (project_id, cause_check_id, effect_check_id, observed_at, co_active, co_resolved, resolution_event_id)
                 VALUES (?1, 'performance.compression', 'performance.lcp', ?2, 1, 0, NULL)",
                rusqlite::params![project_id, i as i64 * 1000],
            ).unwrap();
        }
        // 1 co_resolved
        conn.execute(
            "INSERT INTO causal_link_observations
             (project_id, cause_check_id, effect_check_id, observed_at, co_active, co_resolved, resolution_event_id)
             VALUES (?1, 'performance.compression', 'performance.lcp', 9000, 1, 1, NULL)",
            rusqlite::params![project_id],
        ).unwrap();
        Ok::<(), rusqlite::Error>(())
    })
    .unwrap()
    .unwrap();

    let mut groups = vec![
        make_group("performance.lcp"),
        make_group("performance.compression"),
    ];

    let connected = HashSet::new();
    let dismissed = HashSet::new();
    enrich_issue_groups(
        &mut groups,
        project_id,
        "https://example.com",
        &db,
        &connected,
        &dismissed,
        None,
    )
    .expect("enrich_issue_groups should not fail");

    let lcp_group = groups
        .iter()
        .find(|g| g.check_id == "performance.lcp")
        .expect("lcp group");

    let compression_cause = lcp_group
        .likely_causes
        .iter()
        .find(|c| c.check_id == "performance.compression")
        .expect("compression cause");

    // Base is High (1.0), low ratio drops it: 1.0 - 0.4 = 0.6 = Medium.
    assert_eq!(
        compression_cause.confidence,
        Confidence::Medium,
        "High base with low resolution ratio should drop to Medium"
    );
}

#[test]
fn no_observations_leaves_display_confidence_from_base() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    let db = temp_db();
    let project_id: i64 = 1;
    // No rows seeded - default behavior.

    let mut groups = vec![
        make_group("performance.lcp"),
        make_group("performance.compression"),
    ];

    let connected = HashSet::new();
    let dismissed = HashSet::new();
    enrich_issue_groups(
        &mut groups,
        project_id,
        "https://example.com",
        &db,
        &connected,
        &dismissed,
        None,
    )
    .expect("enrich_issue_groups should not fail");

    let lcp_group = groups
        .iter()
        .find(|g| g.check_id == "performance.lcp")
        .expect("lcp group");

    // With no observations, base confidence (High for compression->lcp) should be unchanged.
    let compression_cause = lcp_group
        .likely_causes
        .iter()
        .find(|c| c.check_id == "performance.compression")
        .expect("compression cause");

    assert_eq!(
        compression_cause.confidence,
        Confidence::High,
        "no observations should leave base confidence unchanged"
    );
    assert_eq!(
        lcp_group.observation_count, 0,
        "observation_count should be 0 when no rows exist"
    );
}

#[test]
fn phase3_fields_populate_from_seeded_db() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    use crate::db::work_items::WorkItemInput;

    let db = temp_db();
    let project_id: i64 = db
        .upsert_project("p1", "https://prod.example.com", None)
        .expect("upsert project");
    let project2_id: i64 = db
        .upsert_project("p2", "https://other.example.com", None)
        .expect("upsert project2");

    db.add_environment(
        project_id,
        "https://prod.example.com",
        "Production",
        "production",
        "manual",
    )
    .expect("add prod env");
    db.add_environment(
        project_id,
        "https://dev.example.com",
        "Dev",
        "development",
        "manual",
    )
    .expect("add dev env");

    let now_ms = chrono::Utc::now().timestamp_millis();
    let five_days_ms = now_ms - 5 * 24 * 60 * 60 * 1000;
    let thirty_days_ms = now_ms - 30 * 24 * 60 * 60 * 1000;

    // Seed work_items for the dev env (5 days ago -- within cross_env 14-day window)
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://dev.example.com",
        vec![WorkItemInput {
            project_id,
            env_url: "https://dev.example.com".into(),
            source: "web_scan".into(),
            signal_id: "web_scan:performance.lcp:https://dev.example.com".into(),
            check_id: "performance.lcp".into(),
            category: "performance".into(),
            severity: Severity::High,
            title: "LCP".into(),
            description: "d".into(),
            detail_json: None,
            scan_ref: None,
            page_url: Some("/home".into()),
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            observed_at: five_days_ms,
            metadata: WorkItemMetadata::default(),
        }],
        five_days_ms,
    )
    .expect("seed dev work item");

    // Seed work_items for project2 (30 days ago -- within cross_project 90-day window)
    db.upsert_work_items_diff(
        "web_scan",
        project2_id,
        "https://other.example.com",
        vec![WorkItemInput {
            project_id: project2_id,
            env_url: "https://other.example.com".into(),
            source: "web_scan".into(),
            signal_id: "web_scan:performance.lcp:https://other.example.com".into(),
            check_id: "performance.lcp".into(),
            category: "performance".into(),
            severity: Severity::High,
            title: "LCP".into(),
            description: "d".into(),
            detail_json: None,
            scan_ref: None,
            page_url: Some("/about".into()),
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            observed_at: thirty_days_ms,
            metadata: WorkItemMetadata::default(),
        }],
        thirty_days_ms,
    )
    .expect("seed project2 work item");

    // Build a group with two page_url instances to verify affected_pages
    let mut lcp_group = make_group("performance.lcp");
    lcp_group.instances = vec![
        IssueInstance {
            id: 0,
            source: "web_scan".into(),
            signal_id: "sig1".into(),
            producer_check_id: None,
            url: None,
            page_url: Some("/home".into()),
            severity: Severity::High,
            title: "LCP".into(),
            description: "d".into(),
            category: None,
            check_status: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            detail_json: None,
            first_seen_at: now_ms,
            last_seen_at: now_ms,
            confidence: None,
            confidence_reason: None,
            domain: None,
            relative_path: None,
            line: None,
            producer_fix_prompt: None,
            producer_category: None,
        },
        IssueInstance {
            id: 1,
            source: "web_scan".into(),
            signal_id: "sig2".into(),
            producer_check_id: None,
            url: None,
            page_url: Some("/pricing".into()),
            severity: Severity::High,
            title: "LCP".into(),
            description: "d".into(),
            category: None,
            check_status: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            detail_json: None,
            first_seen_at: now_ms,
            last_seen_at: now_ms,
            confidence: None,
            confidence_reason: None,
            domain: None,
            relative_path: None,
            line: None,
            producer_fix_prompt: None,
            producer_category: None,
        },
    ];

    let mut groups = vec![lcp_group];
    let connected = HashSet::new();
    let dismissed = HashSet::new();

    enrich_issue_groups(
        &mut groups,
        project_id,
        "https://prod.example.com",
        &db,
        &connected,
        &dismissed,
        None,
    )
    .expect("enrich_issue_groups should not fail");

    let group = &groups[0];

    // affected_pages: flattened, sorted, deduped from instances
    assert_eq!(
        group.affected_pages,
        vec!["/home", "/pricing"],
        "affected_pages should be sorted and deduped"
    );

    // cross_env_signal: prod env sees dev env had the check 5 days ago
    assert!(
        group.cross_env_signal.is_some(),
        "cross_env_signal should be set for prod env when dev had the check recently"
    );
    let env_signal = group.cross_env_signal.as_ref().unwrap();
    assert!(
        env_signal.days_before_prod <= 6,
        "days_before_prod should be ~5; got {}",
        env_signal.days_before_prod
    );

    // cross_project_pattern: project2 also has this check_id within 90 days
    assert!(
        group.cross_project_pattern.is_some(),
        "cross_project_pattern should be set when another project has the check_id"
    );
    assert_eq!(
        group.cross_project_pattern.as_ref().unwrap().project_count,
        1,
        "project_count should be 1 other project"
    );
}

#[test]
fn resolver_skips_v3_when_flag_is_off() {
    // Serialize access because environment variables are process-global.
    let _guard = ENV_VAR_LOCK.write().unwrap();

    // SAFETY: guarded by ENV_VAR_LOCK; unconditionally removed before the lock drops.
    unsafe { std::env::set_var("CORRELATION_V3", "0") };

    let db = temp_db();
    let project_id: i64 = 1;

    let mut groups = vec![
        make_group("performance.lcp"),
        make_group("performance.compression"),
    ];

    let connected = HashSet::new();
    let dismissed = HashSet::new();
    let result = enrich_issue_groups(
        &mut groups,
        project_id,
        "https://example.com",
        &db,
        &connected,
        &dismissed,
        None,
    );

    unsafe { std::env::remove_var("CORRELATION_V3") };
    // _guard drops here, releasing the lock

    result.expect("enrich_issue_groups should not fail with v3 off");

    let lcp_group = groups
        .iter()
        .find(|g| g.check_id == "performance.lcp")
        .expect("lcp group");

    assert!(
        lcp_group.transitive_causes.is_empty(),
        "transitive_causes should be empty when CORRELATION_V3=0; got {:?}",
        lcp_group.transitive_causes
    );
    assert!(
        lcp_group.downstream_effects.is_empty(),
        "downstream_effects should be empty when CORRELATION_V3=0"
    );
    assert!(
        lcp_group.recent_events.is_empty(),
        "recent_events should be empty when CORRELATION_V3=0"
    );
    assert!(
        lcp_group.enrichments.is_empty(),
        "enrichments should be empty when CORRELATION_V3=0"
    );
    // v2 enrichers should still have run
    assert!(
        !lcp_group.likely_causes.is_empty(),
        "likely_causes (v2) should still be populated when CORRELATION_V3=0"
    );
}

#[test]
fn resolver_database_work_is_constant_as_issue_count_grows() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    let db = temp_db();
    let connected = HashSet::new();
    let dismissed = HashSet::new();
    let mut groups: Vec<IssueGroup> = (0..50)
        .map(|index| make_group(&format!("performance.synthetic-{index}")))
        .collect();

    db.reset_operation_count();
    enrich_issue_groups(
        &mut groups,
        1,
        "https://example.com",
        &db,
        &connected,
        &dismissed,
        None,
    )
    .expect("enrich_issue_groups should not fail");

    assert!(
        db.operation_count() <= 7,
        "resolver should preload enrichment indexes instead of querying per issue; operations={}",
        db.operation_count()
    );
}

#[test]
fn resolver_preloads_connected_integration_enrichments_once() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    let db = temp_db();
    let connected = HashSet::from([IntegrationType::GoogleSearchConsole]);
    let dismissed = HashSet::new();
    let mut groups: Vec<IssueGroup> = (0..50).map(|_| make_group("performance.lcp")).collect();

    db.reset_operation_count();
    enrich_issue_groups(
        &mut groups,
        1,
        "https://example.com",
        &db,
        &connected,
        &dismissed,
        None,
    )
    .expect("enrich_issue_groups should not fail");

    assert!(
        db.operation_count() <= 8,
        "connected integration cache should be loaded once per resolver pass; operations={}",
        db.operation_count()
    );
}

#[test]
fn resolver_propagates_observation_storage_failures() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    let db = temp_db();
    db.execute(|conn| conn.execute("DROP TABLE causal_link_observations", []))
        .expect("database worker")
        .expect("drop observation table");
    let mut groups = vec![make_group("performance.lcp")];

    let error = enrich_issue_groups(
        &mut groups,
        1,
        "https://example.com",
        &db,
        &HashSet::new(),
        &HashSet::new(),
        None,
    )
    .expect_err("missing correlation storage must not become empty evidence");

    assert!(error.contains("causal-link observations"));
}

#[test]
fn resolver_propagates_malformed_connected_integration_evidence() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    let db = temp_db_with_project();
    crate::core::correlation::enrichments::write_cache_payload(
        &db,
        1,
        "gsc",
        "field_lcp",
        "not-json",
    )
    .expect("seed malformed cache payload");
    let mut groups = vec![make_group("performance.lcp")];

    let error = enrich_issue_groups(
        &mut groups,
        1,
        "https://example.com",
        &db,
        &HashSet::from([IntegrationType::GoogleSearchConsole]),
        &HashSet::new(),
        None,
    )
    .expect_err("malformed connected evidence must not disappear");

    assert!(error.contains("gsc/field_lcp enrichment payload"));
}

// Enforce a 60 ms p95 budget for a 100-group resolver pass.
#[test]
#[ignore = "perf gate; run with --ignored"]
fn resolver_p95_under_60ms() {
    use crate::core::types_work_items::IssueInstance;

    let dir = tempfile::tempdir().expect("tempdir");
    let db = crate::db::Database::open(dir.path().join("bench.db")).expect("open db");

    let canonical_ids = [
        "performance.compression",
        "performance.lcp",
        "performance.cls",
        "performance.cache_headers",
        "performance.ttfb",
        "performance.render_blocking",
        "performance.unused_javascript",
        "performance.page_weight",
        "seo.indexing.not-indexed",
        "seo.canonical.missing",
    ];

    let build_100_groups = || -> Vec<IssueGroup> {
        canonical_ids
            .iter()
            .cycle()
            .take(100)
            .enumerate()
            .map(|(i, id)| {
                let severity = match i % 4 {
                    0 => "critical",
                    1 => "high",
                    2 => "medium",
                    _ => "low",
                };
                IssueGroup {
                    check_id: id.to_string(),
                    category: "performance".into(),
                    severity: severity.parse().expect("valid severity"),
                    title: format!("Issue {id}"),
                    description: "bench".into(),
                    instances: vec![IssueInstance {
                        id: i as i64,
                        source: "web_scan".into(),
                        signal_id: format!("sig-{i}"),
                        producer_check_id: None,
                        url: Some("https://example.com".into()),
                        page_url: Some("/".into()),
                        severity: severity.parse().expect("valid severity"),
                        title: "bench".into(),
                        description: "bench".into(),
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
                    }],
                    sources: vec!["web_scan".into()],
                    status: crate::core::types_work_items::IssueStatus::New,
                    snooze_until: None,
                    block_reason: None,
                    impact_score: 50.0,
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
            })
            .collect()
    };

    let connected = HashSet::new();
    let dismissed = HashSet::new();
    let mut samples = Vec::with_capacity(100);

    for _ in 0..100 {
        let mut groups = build_100_groups();
        let start = std::time::Instant::now();
        enrich_issue_groups(
            &mut groups,
            1,
            "https://example.com",
            &db,
            &connected,
            &dismissed,
            None,
        )
        .unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }

    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95) / 100];
    let p95_ms = p95 as f64 / 1000.0;
    println!("resolver p95 = {p95_ms:.2}ms");
    assert!(
        p95_ms < 60.0,
        "resolver p95 {p95_ms:.2}ms exceeds 60ms budget"
    );
}

#[test]
fn anomaly_score_surfaces_from_anomaly_event_metadata() {
    let _v3_guard = ENV_VAR_LOCK.read().unwrap();
    use crate::db::types::{EventSeverity, EventSource, EventType, SiteEvent};

    let db = temp_db();
    let project_id: i64 = db
        .upsert_project("anomaly-test", "https://anomaly.example.com", None)
        .expect("upsert project");

    let now_ms = chrono::Utc::now().timestamp_millis();
    let since_ms = now_ms - 30 * 24 * 60 * 60 * 1000;

    // Build a fake AnomalyScore with z = 4.5 and serialize it as metadata.
    let score = crate::core::correlation::anomaly::AnomalyScore {
        z: 4.5,
        current: 145.0,
        mean: 100.0,
        stddev: 10.0,
    };
    let metadata_json = serde_json::to_string(&score).expect("serialize score");

    // Insert an Anomaly event tied to "performance.lcp" via junction table.
    let event = SiteEvent {
        id: 0,
        project_id,
        event_type: EventType::Anomaly,
        severity: EventSeverity::Warning,
        occurred_at_ms: now_ms - 60_000,
        title: "performance.lcp_p75 anomaly: 4.50 from baseline".into(),
        summary: "Current 145.00, baseline 100.00 +/- 10.00".into(),
        detail: None,
        source: EventSource::Internal,
        source_id: Some(format!("anomaly_performance.lcp_p75_{}", now_ms - 60_000)),
        metadata: Some(metadata_json),
        affected_check_ids: Some(vec!["performance.lcp".to_string()]),
    };
    db.insert_event(&event).expect("insert anomaly event");

    // Enrich a group with check_id "performance.lcp".
    let mut groups = vec![make_group("performance.lcp")];
    let connected = HashSet::new();
    let dismissed = HashSet::new();

    // Patch the events_by_check_id lookup by using the real DB path.
    // We need since_ms < now - 60s, so the event is in the window.
    let _ = since_ms;

    enrich_issue_groups(
        &mut groups,
        project_id,
        "https://anomaly.example.com",
        &db,
        &connected,
        &dismissed,
        None,
    )
    .expect("enrich should not fail");

    let group = &groups[0];
    assert!(
        group.anomaly_score.is_some(),
        "anomaly_score should be populated from the Anomaly event metadata"
    );
    let z = group.anomaly_score.unwrap();
    assert!(
        (z - 4.5).abs() < 0.01,
        "anomaly_score z should be ~4.5, got {z}"
    );
}
