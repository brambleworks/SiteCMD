use super::*;
use crate::integrations::adapters::{AdapterError, IntegrationAdapter, PollContext, PollOutput};
use async_trait::async_trait;

struct NoopAdapter;

#[async_trait]
impl IntegrationAdapter for NoopAdapter {
    fn source(&self) -> &'static str {
        "noop"
    }
    fn cadence(&self) -> Duration {
        Duration::from_secs(60)
    }
    async fn poll(&self, _ctx: &PollContext) -> Result<PollOutput, AdapterError> {
        Ok(PollOutput::default())
    }
}

#[tokio::test]
async fn scheduler_constructs_with_adapters() {
    let scheduler = IntegrationScheduler::new(vec![Arc::new(NoopAdapter)]);
    assert_eq!(scheduler.adapters.len(), 1);
}

#[tokio::test]
async fn immediate_sender_can_be_cloned_and_sent() {
    let scheduler = IntegrationScheduler::new(vec![Arc::new(NoopAdapter)]);
    let tx = scheduler.immediate_sender();
    tx.send(ImmediateRequest {
        source: "noop".into(),
        project_id: 1,
        env_url: None,
    })
    .unwrap();
}

#[tokio::test]
async fn immediate_worker_serializes_requests_without_blocking_the_caller() {
    let (tx, rx) = mpsc::unbounded_channel();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let gate = Arc::new(tokio::sync::Notify::new());

    let worker = {
        let log = log.clone();
        let gate = gate.clone();
        spawn_immediate_worker(rx, move |req| {
            let log = log.clone();
            let gate = gate.clone();
            async move {
                log.lock().await.push(format!("start:{}", req.project_id));
                if req.project_id == 1 {
                    gate.notified().await;
                }
                log.lock().await.push(format!("end:{}", req.project_id));
            }
        })
    };

    for project_id in [1, 2] {
        tx.send(ImmediateRequest {
            source: "noop".into(),
            project_id,
            env_url: None,
        })
        .unwrap();
    }

    // Wait for the worker to pick up the first (gated) request.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !log.lock().await.iter().any(|e| e == "start:1") {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("worker should start the first request");

    // The first request is still in flight, so the second must not start.
    assert!(
        !log.lock().await.iter().any(|e| e == "start:2"),
        "second request ran while the first was in flight"
    );

    gate.notify_one();
    drop(tx);
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("worker should exit once the channel closes")
        .expect("worker task should not panic");

    assert_eq!(
        *log.lock().await,
        vec!["start:1", "end:1", "start:2", "end:2"]
    );
}

#[test]
fn send_immediate_request_errors_when_uninitialized() {
    // An explicit sender avoids the process-global OnceLock.
    let err = send_immediate_request(None, "noop", 1, None).unwrap_err();
    assert!(err.contains("not initialized"), "got: {err}");
}

#[test]
fn send_immediate_request_delivers_when_initialized() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    send_immediate_request(Some(&tx), "noop", 7, Some("https://example.com")).unwrap();
    let req = rx.try_recv().expect("request delivered");
    assert_eq!(req.source, "noop");
    assert_eq!(req.project_id, 7);
    assert_eq!(req.env_url.as_deref(), Some("https://example.com"));
}

#[test]
fn source_to_service_type_maps_known_sources() {
    assert_eq!(source_to_service_type("uptimerobot"), Some("uptimerobot"));
    assert_eq!(source_to_service_type("plausible"), Some("plausible"));
    assert_eq!(source_to_service_type("cloudflare"), Some("cloudflare"));
    assert_eq!(source_to_service_type("gsc"), Some("googlesearchconsole"));
    assert_eq!(source_to_service_type("ga4"), Some("googleanalytics"));
    // Sources with no DB credentials short-circuit before touching app/db.
    assert_eq!(source_to_service_type("psi"), None);
    assert_eq!(source_to_service_type("updates"), None);
    assert_eq!(source_to_service_type("unknown"), None);
    assert_eq!(source_to_service_type(""), None);
}

#[test]
fn first_env_per_project_keeps_only_the_leading_env_for_each_project() {
    // Input arrives production-first per project (list_all_project_envs
    // orders it that way), so keeping the first env keeps production.
    let envs = vec![
        (1, "https://one.example.com".to_string()),
        (1, "http://localhost:3000".to_string()),
        (2, "https://two.example.com".to_string()),
        (3, "https://three.example.com".to_string()),
        (3, "http://localhost:5173".to_string()),
    ];

    let collapsed = first_env_per_project(&envs);

    assert_eq!(
        collapsed,
        vec![
            (1, "https://one.example.com".to_string()),
            (2, "https://two.example.com".to_string()),
            (3, "https://three.example.com".to_string()),
        ]
    );
}

#[test]
fn credential_scoped_adapters_opt_out_of_env_fanout() {
    let db = crate::db::test_helpers::temp_db_arc();
    assert!(
        !crate::integrations::adapters::cloudflare_adapter::CloudflareAdapter::new(db.db.clone())
            .env_scoped()
    );
    assert!(!crate::integrations::adapters::ga4_adapter::Ga4Adapter::new().env_scoped());
    assert!(
        !crate::integrations::adapters::plausible_adapter::PlausibleAdapter::new(db.db.clone())
            .env_scoped()
    );
}

#[test]
fn mark_due_or_seed_skips_the_first_automatic_poll() {
    let mut last_run = HashMap::new();
    let key = ("updates".to_string(), 7, "https://example.com".to_string());
    let now = Instant::now();

    let due = mark_due_or_seed(&mut last_run, key.clone(), now, Duration::from_secs(60));

    assert!(!due);
    assert_eq!(last_run.get(&key).copied(), Some(now));
}

#[test]
fn mark_due_or_seed_runs_after_cadence_elapses() {
    let mut last_run = HashMap::new();
    let key = (
        "plausible".to_string(),
        9,
        "https://example.com".to_string(),
    );
    let first = Instant::now();
    last_run.insert(key.clone(), first);
    let later = first + Duration::from_secs(61);

    let due = mark_due_or_seed(&mut last_run, key.clone(), later, Duration::from_secs(60));

    assert!(due);
    assert_eq!(last_run.get(&key).copied(), Some(later));
}

fn updates_item(
    project_id: i64,
    env_url: &str,
    signal_id: &str,
) -> crate::db::work_items::WorkItemInput {
    crate::db::work_items::WorkItemInput {
        project_id,
        env_url: env_url.to_string(),
        source: "updates".to_string(),
        signal_id: signal_id.to_string(),
        check_id: "updates-check".to_string(),
        category: "dependencies".to_string(),
        severity: crate::checks::Severity::High,
        title: signal_id.to_string(),
        description: String::new(),
        detail_json: None,
        scan_ref: None,
        page_url: None,
        fix_prompt: None,
        manual_fix: None,
        why_it_matters: None,
        observed_at: 1_000,
        metadata: crate::db::work_items::WorkItemMetadata::default(),
    }
}

fn seed_two_family_items(db: &Database) -> (i64, &'static str) {
    let env_url = "http://example.com";
    let project_id = db.upsert_project("poll-apply", "", None).expect("project");
    db.upsert_work_items_diff(
        "updates",
        project_id,
        env_url,
        vec![
            updates_item(project_id, env_url, "updates:vulnerability:npm:left-pad"),
            updates_item(project_id, env_url, "updates:ssl-expiring:example.com"),
        ],
        1_000,
    )
    .expect("seed items");
    (project_id, env_url)
}

fn active_signal_ids(db: &Database, project_id: i64, env_url: &str) -> Vec<String> {
    db.get_active_work_items(project_id, Some(env_url))
        .expect("active work items")
        .into_iter()
        .map(|item| item.signal_id)
        .collect()
}

#[test]
fn apply_poll_output_scoped_partial_preserves_only_the_unobserved_family() {
    let db = crate::db::test_helpers::temp_db();
    let (project_id, env_url) = seed_two_family_items(&db);

    apply_poll_output(
        &db,
        "updates",
        project_id,
        env_url,
        PollOutput {
            work_items: vec![],
            alerts: vec![],
            partial: false,
            unobserved_signal_prefixes: vec!["updates:vulnerability:".to_string()],
        },
        2_000,
    );

    let active = active_signal_ids(&db, project_id, env_url);
    assert!(
        active.contains(&"updates:vulnerability:npm:left-pad".to_string()),
        "the unobserved dependency family must survive, got: {active:?}"
    );
    assert!(
        !active.contains(&"updates:ssl-expiring:example.com".to_string()),
        "the observed-and-absent SSL item must diff-resolve, got: {active:?}"
    );
}

#[test]
fn apply_poll_output_whole_source_partial_resolves_nothing() {
    let db = crate::db::test_helpers::temp_db();
    let (project_id, env_url) = seed_two_family_items(&db);

    apply_poll_output(
        &db,
        "updates",
        project_id,
        env_url,
        PollOutput {
            work_items: vec![],
            alerts: vec![],
            partial: true,
            unobserved_signal_prefixes: vec!["updates:vulnerability:".to_string()],
        },
        2_000,
    );

    let active = active_signal_ids(&db, project_id, env_url);
    assert_eq!(active.len(), 2, "a fully partial poll resolves nothing");
}

#[test]
fn apply_poll_output_scoped_partial_still_inserts_observed_findings() {
    let db = crate::db::test_helpers::temp_db();
    let (project_id, env_url) = seed_two_family_items(&db);

    apply_poll_output(
        &db,
        "updates",
        project_id,
        env_url,
        PollOutput {
            work_items: vec![updates_item(
                project_id,
                env_url,
                "updates:vulnerability:npm:lodash",
            )],
            alerts: vec![],
            partial: false,
            unobserved_signal_prefixes: vec!["updates:vulnerability:".to_string()],
        },
        2_000,
    );

    let active = active_signal_ids(&db, project_id, env_url);
    assert!(
        active.contains(&"updates:vulnerability:npm:lodash".to_string()),
        "a newly observed finding must insert even in a partial family, got: {active:?}"
    );
    assert!(
        active.contains(&"updates:vulnerability:npm:left-pad".to_string()),
        "the unobserved sibling must survive, got: {active:?}"
    );
}

#[test]
fn apply_poll_output_complete_poll_diff_resolves_everything() {
    // Baseline guard: with nothing unobserved, an empty complete poll
    // resolves both families exactly as before.
    let db = crate::db::test_helpers::temp_db();
    let (project_id, env_url) = seed_two_family_items(&db);

    apply_poll_output(
        &db,
        "updates",
        project_id,
        env_url,
        PollOutput::default(),
        2_000,
    );

    assert!(
        active_signal_ids(&db, project_id, env_url).is_empty(),
        "a complete empty poll must resolve all active items"
    );
}

// github_context_from_configs: not-configured vs unobservable

fn github_config(
    api_key: Option<&str>,
    site_id: Option<&str>,
    extra: Option<serde_json::Value>,
) -> crate::integrations::IntegrationConfig {
    crate::integrations::IntegrationConfig {
        integration_type: crate::integrations::IntegrationType::GitHub,
        api_key: api_key.map(str::to_string),
        site_id: site_id.map(str::to_string),
        extra,
        enabled: true,
    }
}

// Keyring closure for paths that must never consult the keyring.
fn keyring_untouched() -> Result<Option<String>, String> {
    panic!("keyring must not be consulted on this path");
}

#[test]
fn github_context_no_integration_is_not_configured() {
    // Deliberate absence: CI items should resolve normally, and neither
    // keyring lookup runs.
    let resolution = github_context_from_configs(vec![], keyring_untouched, keyring_untouched);
    assert!(matches!(resolution, GithubContextResolution::NotConfigured));
}

#[test]
fn github_context_resolves_a_stored_pat() {
    let resolution = github_context_from_configs(
        vec![github_config(Some("ghp_token"), Some("acme/site"), None)],
        keyring_untouched,
        keyring_untouched,
    );
    let GithubContextResolution::Resolved(gh) = resolution else {
        panic!("expected Resolved, got {resolution:?}");
    };
    assert_eq!(gh.owner, "acme");
    assert_eq!(gh.repo, "site");
    assert_eq!(gh.token, "ghp_token");
}

#[test]
fn github_context_keyring_api_key_error_is_unobservable() {
    let resolution = github_context_from_configs(
        vec![github_config(
            Some(crate::keyring::KEYRING_PLACEHOLDER),
            Some("acme/site"),
            None,
        )],
        || Err("keychain locked".to_string()),
        keyring_untouched,
    );
    assert!(
        matches!(resolution, GithubContextResolution::Unobservable),
        "a keyring API key error must be unobservable, got {resolution:?}"
    );
}

#[test]
fn github_context_keyring_tokens_error_is_unobservable() {
    // Same hole on the OAuth path: no PAT, no token in extra, and the
    // keyring tokens read fails.
    let resolution = github_context_from_configs(
        vec![github_config(None, Some("acme/site"), None)],
        keyring_untouched,
        || Err("keychain locked".to_string()),
    );
    assert!(
        matches!(resolution, GithubContextResolution::Unobservable),
        "a keyring tokens error must be unobservable, got {resolution:?}"
    );
}

#[test]
fn github_context_missing_credential_at_rest_stays_not_configured() {
    // The keyring answered (no error) and has nothing stored: an
    // at-rest state that keeps the pre-existing skip-and-resolve behavior.
    let resolution = github_context_from_configs(
        vec![github_config(
            Some(crate::keyring::KEYRING_PLACEHOLDER),
            Some("acme/site"),
            None,
        )],
        || Ok(None),
        || Ok(None),
    );
    assert!(matches!(resolution, GithubContextResolution::NotConfigured));
}

#[test]
fn github_context_oauth_token_from_extra_never_touches_keyring() {
    let extra = serde_json::json!({"tokens": {"access_token": "gho_token"}});
    let resolution = github_context_from_configs(
        vec![github_config(None, Some("acme/site"), Some(extra))],
        keyring_untouched,
        keyring_untouched,
    );
    let GithubContextResolution::Resolved(gh) = resolution else {
        panic!("expected Resolved, got {resolution:?}");
    };
    assert_eq!(gh.token, "gho_token");
}

#[test]
fn github_context_malformed_repo_spec_is_not_configured() {
    // No "owner/repo" means nothing to observe by construction - the
    // pre-existing skip path, byte-identical.
    for site_id in [None, Some(""), Some("not-a-repo-spec")] {
        let resolution = github_context_from_configs(
            vec![github_config(Some("ghp_token"), site_id, None)],
            keyring_untouched,
            keyring_untouched,
        );
        assert!(
            matches!(resolution, GithubContextResolution::NotConfigured),
            "site_id {site_id:?} must be NotConfigured, got {resolution:?}"
        );
    }
}

#[test]
fn free_cadence_is_six_times_base() {
    use std::time::Duration;
    let base = Duration::from_secs(60);
    assert_eq!(
        tier_adjusted_cadence(base, crate::licensing::config::Tier::Free),
        Duration::from_secs(360)
    );
}

#[test]
fn paid_cadence_is_unchanged() {
    use std::time::Duration;
    let base = Duration::from_secs(3600);
    assert_eq!(
        tier_adjusted_cadence(base, crate::licensing::config::Tier::Core),
        base
    );
    assert_eq!(
        tier_adjusted_cadence(base, crate::licensing::config::Tier::Pro),
        base
    );
}

// credentials_from_configs: a plaintext SQLite credential left by a failed
// keyring migration must never reach an adapter's outbound poll. This is the
// pure core `resolve_credentials` delegates to (mirroring how
// `github_context_from_configs` separates from `resolve_github_context`), so
// it is testable without a live `AppHandle`.

/// Counts audit-log lines matching both an op and an integration, so the
/// assertion is immune to unrelated entries other tests append to the same
/// process-wide file. These tests run as plain `#[test]` (no tokio runtime),
/// so `audit_log::record`'s `Handle::try_current()` check fails and the write
/// happens inline before `credentials_from_configs` returns - no race to poll
/// for.
fn count_audit_log_entries_for(op: &str, integration: &str) -> usize {
    let Some(path) = crate::app_identity::default_storage_dir().map(|dir| dir.join("audit.log"))
    else {
        return 0;
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return 0;
    };
    let needle_op = format!(r#""op":"{op}""#);
    let needle_integration = format!(r#""integration":"{integration}""#);
    contents
        .lines()
        .filter(|line| line.contains(&needle_op) && line.contains(&needle_integration))
        .count()
}

#[test]
fn credentials_from_configs_refuses_unmigrated_plaintext_api_key() {
    use crate::integrations::{IntegrationConfig, IntegrationType};

    let config = IntegrationConfig {
        integration_type: IntegrationType::Plausible,
        api_key: Some("plausible-live-secret-value".to_string()),
        site_id: Some("sitecmd.com".to_string()),
        extra: None,
        enabled: true,
    };

    let before = count_audit_log_entries_for("credential_refused_unmigrated", "plausible");

    // get_api_key panics if called: once refused, the api_key no longer
    // equals the placeholder, so that hydration path must never run. There is
    // no `extra.tokens` here, so get_tokens still runs as the normal fallback
    // and simulates the keychain having nothing either (the migration never
    // completed, which is why the plaintext value was still in SQLite).
    let creds = credentials_from_configs(
        vec![config],
        IntegrationType::Plausible,
        None,
        false,
        keyring_untouched,
        || Ok(None),
    );

    assert_eq!(
        creds.api_key, None,
        "a plaintext SQLite api_key must never reach the adapter poll"
    );
    let after = count_audit_log_entries_for("credential_refused_unmigrated", "plausible");
    assert!(
        after > before,
        "expected a new credential_refused_unmigrated audit entry for plausible"
    );
}

#[test]
fn credentials_from_configs_refuses_unmigrated_plaintext_oauth_token() {
    use crate::integrations::{IntegrationConfig, IntegrationType};

    let config = IntegrationConfig {
        integration_type: IntegrationType::GoogleSearchConsole,
        api_key: None,
        site_id: Some("https://sitecmd.com/".to_string()),
        extra: Some(serde_json::json!({
            "tokens": { "access_token": "gsc-live-oauth-secret" }
        })),
        enabled: true,
    };

    let before =
        count_audit_log_entries_for("credential_refused_unmigrated", "googlesearchconsole");

    // get_api_key panics if called: there is no api_key on this config, so
    // the placeholder-hydration path must never run. get_tokens simulates
    // the keychain having nothing either, matching a migration that never
    // completed (the reason the plaintext token was still in SQLite).
    let creds = credentials_from_configs(
        vec![config],
        IntegrationType::GoogleSearchConsole,
        None,
        false,
        keyring_untouched,
        || Ok(None),
    );

    assert_eq!(
        creds.oauth_token, None,
        "a plaintext SQLite OAuth token must never reach the adapter poll"
    );
    let after = count_audit_log_entries_for("credential_refused_unmigrated", "googlesearchconsole");
    assert!(
        after > before,
        "expected a new credential_refused_unmigrated audit entry for googlesearchconsole"
    );
}
