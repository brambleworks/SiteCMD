use super::*;
use crate::db::test_helpers::temp_db_arc as temp_db;
use crate::updates::types::{Ecosystem, PackageUpdate};

// Whether every dependency-derived family was unobservable.
fn dependency_families_unobserved(out: &PollOutput) -> bool {
    DEPENDENCY_SIGNAL_PREFIXES
        .iter()
        .all(|prefix| out.unobserved_signal_prefixes.iter().any(|p| p == prefix))
}

#[test]
fn source_is_updates() {
    let db = temp_db();
    let adapter = UpdatesAdapter::new(db.db.clone());
    assert_eq!(adapter.source(), "updates");
}

#[test]
fn cadence_is_1_hour() {
    let db = temp_db();
    let adapter = UpdatesAdapter::new(db.db.clone());
    assert_eq!(adapter.cadence(), Duration::from_secs(3600));
}

#[tokio::test]
async fn returns_empty_when_no_project_path() {
    let db = temp_db();
    let project_id = db
        .upsert_project("No-path project", "", None)
        .expect("upsert project");

    let adapter = UpdatesAdapter::new(db.db.clone());
    let ctx = PollContext {
        project_id,
        env_url: "http://example.com".into(),
        detected_stack: None,
        credentials: crate::integrations::adapters::Credentials::empty(),
    };
    let result = adapter.poll(&ctx).await.expect("should not error");
    assert!(result.work_items.is_empty());
    assert!(result.alerts.is_empty());
    assert!(
        !result.partial,
        "the scoped path never flags the whole poll"
    );
    assert!(
        dependency_families_unobserved(&result),
        "an unreadable project path must report the dependency families unobserved, got: {:?}",
        result.unobserved_signal_prefixes
    );
    assert!(
        !result
            .unobserved_signal_prefixes
            .iter()
            .any(|p| p == SSL_SIGNAL_PREFIX || p == CI_SIGNAL_PREFIX),
        "SSL/CI families must stay resolvable when only the folder is unreadable, got: {:?}",
        result.unobserved_signal_prefixes
    );
}

#[tokio::test]
async fn ssl_probe_failure_marks_only_the_ssl_family_unobserved() {
    // A failed SSL probe must not freeze successfully observed dependency families.
    let db = temp_db();
    let dir = tempfile::tempdir().expect("tempdir");
    let project_id = db
        .upsert_project("with-folder", dir.path().to_str().unwrap(), None)
        .expect("upsert project");

    let adapter = UpdatesAdapter::new(db.db.clone());
    let ctx = PollContext {
        project_id,
        env_url: "not a url".into(),
        detected_stack: None,
        credentials: crate::integrations::adapters::Credentials::empty(),
    };
    let result = adapter.poll(&ctx).await.expect("should not error");
    assert_eq!(
        result.unobserved_signal_prefixes,
        vec![SSL_SIGNAL_PREFIX.to_string()],
        "an SSL probe failure must mark exactly the ssl-expiring family unobserved"
    );
    assert!(!result.partial);
}

// An active `updates` work item as an earlier, healthy poll would have
// tracked it, for the observability regression tests below.
fn seeded_issue(
    project_id: i64,
    env_url: &str,
    signal_id: &str,
    check_kind: &str,
    observed_at: i64,
) -> WorkItemInput {
    WorkItemInput {
        project_id,
        env_url: env_url.to_string(),
        source: "updates".to_string(),
        signal_id: signal_id.to_string(),
        check_id: resolve_check_id("updates", check_kind),
        category: "dependencies".to_string(),
        severity: Severity::High,
        title: signal_id.to_string(),
        description: format!("{} is active.", signal_id),
        detail_json: None,
        scan_ref: None,
        page_url: None,
        fix_prompt: None,
        manual_fix: None,
        why_it_matters: None,
        observed_at,
        metadata: WorkItemMetadata::default(),
    }
}

// An active dependency work item as an earlier, healthy scan would have
// tracked it, for the lockfile-observability regression tests below.
fn seeded_dependency_issue(project_id: i64, env_url: &str, observed_at: i64) -> WorkItemInput {
    seeded_issue(
        project_id,
        env_url,
        "updates:vulnerability:npm:left-pad",
        "vulnerability",
        observed_at,
    )
}

#[tokio::test]
async fn degraded_dependency_census_preserves_dependency_items_but_resolves_ssl_items() {
    let db = temp_db();
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"dependencies": {"left-pad": "^1.0.0"}}"#,
    )
    .expect("write package.json");
    let lockfile =
        std::fs::File::create(dir.path().join("package-lock.json")).expect("create lockfile");
    lockfile
        .set_len(crate::constants::MAX_DEPENDENCY_FILE_BYTES + 1)
        .expect("grow sparse file");

    let project_id = db
        .upsert_project("oversized-lockfile", dir.path().to_str().unwrap(), None)
        .expect("upsert project");
    let env_url = "http://example.com";
    db.upsert_work_items_diff(
        "updates",
        project_id,
        env_url,
        vec![
            seeded_dependency_issue(project_id, env_url, 1_000),
            seeded_issue(
                project_id,
                env_url,
                "updates:ssl-expiring:example.com",
                "ssl-expiring",
                1_000,
            ),
        ],
        1_000,
    )
    .expect("seed issues");

    let adapter = UpdatesAdapter::new(db.db.clone());
    let ctx = PollContext {
        project_id,
        env_url: env_url.into(),
        detected_stack: None,
        credentials: crate::integrations::adapters::Credentials::empty(),
    };
    let result = adapter.poll(&ctx).await.expect("should not error");
    assert!(
        dependency_families_unobserved(&result),
        "a present-but-oversized lockfile must mark the dependency families unobserved, got: {:?}",
        result.unobserved_signal_prefixes
    );
    assert!(
        !result
            .unobserved_signal_prefixes
            .iter()
            .any(|p| p == SSL_SIGNAL_PREFIX || p == CI_SIGNAL_PREFIX),
        "SSL/CI observability is independent of the lockfile, got: {:?}",
        result.unobserved_signal_prefixes
    );
    assert!(!result.partial);

    // Apply the poll exactly the way the scheduler does.
    crate::core::integration_scheduler::apply_poll_output(
        &db, "updates", project_id, env_url, result, 2_000,
    );
    let active = db
        .get_active_work_items(project_id, Some(env_url))
        .expect("active work items");
    assert!(
        active
            .iter()
            .any(|item| item.signal_id == "updates:vulnerability:npm:left-pad"),
        "a previously-tracked dependency issue must survive an unreadable-lockfile tick"
    );
    assert!(
        !active
            .iter()
            .any(|item| item.signal_id == "updates:ssl-expiring:example.com"),
        "an observed-and-absent SSL issue must still resolve on a degraded dependency tick"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_plugins_dir_preserves_plugin_vulnerability_items() {
    use std::os::unix::fs::PermissionsExt;
    let db = temp_db();
    let dir = tempfile::tempdir().expect("tempdir");
    let plugins = dir.path().join("wp-content/plugins");
    std::fs::create_dir_all(&plugins).expect("create plugins dir");
    std::fs::set_permissions(&plugins, std::fs::Permissions::from_mode(0o000))
        .expect("chmod plugins dir");
    if std::fs::read_dir(&plugins).is_ok() {
        // Permissions not enforced (e.g. running as root): skip.
        std::fs::set_permissions(&plugins, std::fs::Permissions::from_mode(0o755))
            .expect("restore plugins dir");
        return;
    }

    let project_id = db
        .upsert_project("unreadable-plugins", dir.path().to_str().unwrap(), None)
        .expect("upsert project");
    let env_url = "http://example.com";
    db.upsert_work_items_diff(
        "updates",
        project_id,
        env_url,
        vec![seeded_issue(
            project_id,
            env_url,
            "updates:vulnerability:wordpress:akismet",
            "vulnerability",
            1_000,
        )],
        1_000,
    )
    .expect("seed plugin vulnerability");

    let adapter = UpdatesAdapter::new(db.db.clone());
    let ctx = PollContext {
        project_id,
        env_url: env_url.into(),
        detected_stack: None,
        credentials: crate::integrations::adapters::Credentials::empty(),
    };
    let result = adapter.poll(&ctx).await.expect("should not error");
    std::fs::set_permissions(&plugins, std::fs::Permissions::from_mode(0o755))
        .expect("restore plugins dir");
    assert!(
        dependency_families_unobserved(&result),
        "an unreadable plugins directory must mark the dependency families unobserved, got: {:?}",
        result.unobserved_signal_prefixes
    );

    crate::core::integration_scheduler::apply_poll_output(
        &db, "updates", project_id, env_url, result, 2_000,
    );
    let active = db
        .get_active_work_items(project_id, Some(env_url))
        .expect("active work items");
    assert!(
        active
            .iter()
            .any(|item| item.signal_id == "updates:vulnerability:wordpress:akismet"),
        "a previously-tracked plugin vulnerability must survive an unreadable-plugins-dir tick"
    );
}

#[tokio::test]
async fn absent_lockfile_still_resolves_previously_tracked_dependency_issues() {
    let db = temp_db();
    let dir = tempfile::tempdir().expect("tempdir");

    let project_id = db
        .upsert_project("absent-lockfile", dir.path().to_str().unwrap(), None)
        .expect("upsert project");
    let env_url = "http://example.com";
    db.upsert_work_items_diff(
        "updates",
        project_id,
        env_url,
        vec![seeded_dependency_issue(project_id, env_url, 1_000)],
        1_000,
    )
    .expect("seed dependency issue");

    let adapter = UpdatesAdapter::new(db.db.clone());
    let ctx = PollContext {
        project_id,
        env_url: env_url.into(),
        detected_stack: None,
        credentials: crate::integrations::adapters::Credentials::empty(),
    };
    let result = adapter.poll(&ctx).await.expect("should not error");
    assert!(
        !result.partial && result.unobserved_signal_prefixes.is_empty(),
        "an empty readable folder is a complete observation, got: {:?}",
        result.unobserved_signal_prefixes
    );

    db.upsert_work_items_diff("updates", project_id, env_url, result.work_items, 2_000)
        .expect("diff upsert");
    let active = db
        .get_active_work_items(project_id, Some(env_url))
        .expect("active work items");
    assert!(
        !active
            .iter()
            .any(|item| item.signal_id == "updates:vulnerability:npm:left-pad"),
        "an absent lockfile must still diff-resolve stale dependency issues"
    );
}

#[tokio::test]
async fn unresolvable_github_context_preserves_ci_items() {
    let db = temp_db();
    let dir = tempfile::tempdir().expect("tempdir");
    let project_id = db
        .upsert_project("ci-unobservable", dir.path().to_str().unwrap(), None)
        .expect("upsert project");
    let env_url = "http://example.com";
    db.upsert_work_items_diff(
        "updates",
        project_id,
        env_url,
        vec![seeded_issue(
            project_id,
            env_url,
            "updates:ci-failure:CI:123",
            "ci-failure",
            1_000,
        )],
        1_000,
    )
    .expect("seed CI issue");

    let adapter = UpdatesAdapter::new(db.db.clone());
    let ctx = PollContext {
        project_id,
        env_url: env_url.into(),
        detected_stack: None,
        // github: None + github_unobservable: the scheduler resolved a
        // configured GitHub integration into a failure this pass.
        credentials: crate::integrations::adapters::Credentials {
            github_unobservable: true,
            ..crate::integrations::adapters::Credentials::empty()
        },
    };
    let result = adapter.poll(&ctx).await.expect("should not error");
    assert_eq!(
        result.unobserved_signal_prefixes,
        vec![CI_SIGNAL_PREFIX.to_string()],
        "a failed GitHub context resolution must mark exactly the ci-failure family unobserved"
    );
    assert!(!result.partial);

    crate::core::integration_scheduler::apply_poll_output(
        &db, "updates", project_id, env_url, result, 2_000,
    );
    let active = db
        .get_active_work_items(project_id, Some(env_url))
        .expect("active work items");
    assert!(
        active
            .iter()
            .any(|item| item.signal_id == "updates:ci-failure:CI:123"),
        "an active CI item must survive a tick whose GitHub context could not be resolved"
    );
}

#[tokio::test]
async fn github_not_configured_resolves_stale_ci_items() {
    let db = temp_db();
    let dir = tempfile::tempdir().expect("tempdir");
    let project_id = db
        .upsert_project("ci-not-configured", dir.path().to_str().unwrap(), None)
        .expect("upsert project");
    let env_url = "http://example.com";
    db.upsert_work_items_diff(
        "updates",
        project_id,
        env_url,
        vec![seeded_issue(
            project_id,
            env_url,
            "updates:ci-failure:CI:123",
            "ci-failure",
            1_000,
        )],
        1_000,
    )
    .expect("seed CI issue");

    let adapter = UpdatesAdapter::new(db.db.clone());
    let ctx = PollContext {
        project_id,
        env_url: env_url.into(),
        detected_stack: None,
        credentials: crate::integrations::adapters::Credentials::empty(),
    };
    let result = adapter.poll(&ctx).await.expect("should not error");
    assert!(
        result.unobserved_signal_prefixes.is_empty(),
        "GitHub deliberately not configured must not mark anything unobserved, got: {:?}",
        result.unobserved_signal_prefixes
    );

    crate::core::integration_scheduler::apply_poll_output(
        &db, "updates", project_id, env_url, result, 2_000,
    );
    let active = db
        .get_active_work_items(project_id, Some(env_url))
        .expect("active work items");
    assert!(
        !active
            .iter()
            .any(|item| item.signal_id == "updates:ci-failure:CI:123"),
        "a stale CI item must resolve when GitHub is deliberately not configured"
    );
}

#[tokio::test]
async fn returns_empty_for_unknown_project_id() {
    let db = temp_db();
    let adapter = UpdatesAdapter::new(db.db.clone());
    let ctx = PollContext {
        project_id: 99999,
        env_url: "http://example.com".into(),
        detected_stack: None,
        credentials: crate::integrations::adapters::Credentials::empty(),
    };
    let result = adapter.poll(&ctx).await.expect("should not error");
    assert!(result.work_items.is_empty());
    assert!(dependency_families_unobserved(&result));
    assert!(!result.partial);
}

#[test]
fn critical_security_updates_emit_alerts() {
    let pkg = PackageUpdate {
        name: "axios".into(),
        current_version: "1.6.0".into(),
        latest_version: "1.7.4".into(),
        ecosystem: Ecosystem::Npm,
        update_type: UpdateType::Patch,
        is_security: true,
        advisory_severity: Some("critical".to_string()),
        advisory_url: Some("https://osv.dev/GHSA-test".into()),
        source: "package-lock.json".into(),
        is_dev: false,
        ..Default::default()
    };

    let alert = build_security_update_alert(7, "https://example.com", &pkg, 1_000).expect("alert");

    assert_eq!(alert.source, "updates");
    assert_eq!(alert.severity, "critical");
    assert!(alert.alert_id.contains("security-update:npm:axios"));
    assert_eq!(alert.env_url, None);
    assert!(alert.description.contains("no verified fixed release"));
    let detail: serde_json::Value =
        serde_json::from_str(alert.detail_json.as_deref().expect("security alert detail"))
            .expect("valid security alert detail");
    assert!(detail["advisory_fixed_version"].is_null());
}

#[test]
fn verified_security_release_is_used_in_alert_guidance() {
    let pkg = PackageUpdate {
        name: "axios".into(),
        current_version: "1.6.0".into(),
        latest_version: "1.7.4".into(),
        ecosystem: Ecosystem::Npm,
        update_type: UpdateType::Patch,
        is_security: true,
        advisory_severity: Some("high".to_string()),
        advisory_fixed_version: Some("1.7.4".to_string()),
        source: "package-lock.json".into(),
        ..Default::default()
    };

    let alert = build_security_update_alert(7, "https://example.com", &pkg, 1_000).expect("alert");

    assert!(alert.description.contains("Update to 1.7.4"));
    let detail: serde_json::Value =
        serde_json::from_str(alert.detail_json.as_deref().expect("security alert detail"))
            .expect("valid security alert detail");
    assert_eq!(detail["advisory_fixed_version"], "1.7.4");
}

#[test]
fn medium_security_updates_do_not_emit_alerts() {
    let pkg = PackageUpdate {
        name: "dev-tool".into(),
        current_version: "1.0.0".into(),
        latest_version: "1.0.1".into(),
        ecosystem: Ecosystem::Npm,
        update_type: UpdateType::Patch,
        is_security: true,
        advisory_severity: Some("medium".to_string()),
        advisory_url: None,
        source: "package-lock.json".into(),
        is_dev: true,
        ..Default::default()
    };

    assert!(build_security_update_alert(7, "https://example.com", &pkg, 1_000).is_none());
}

#[test]
fn deprecated_packages_become_medium_work_items_with_message() {
    let pkg = PackageUpdate {
        name: "request".into(),
        current_version: "2.88.2".into(),
        latest_version: "2.88.2".into(),
        ecosystem: Ecosystem::Npm,
        source: "package-lock.json".into(),
        is_deprecated: true,
        deprecation_message: Some("request has been deprecated, see issue #3142".into()),
        ..Default::default()
    };

    let item = build_deprecated_work_item(7, "https://example.com".into(), &pkg, 1_000);

    assert_eq!(item.source, "updates");
    assert_eq!(item.signal_id, "updates:deprecated:npm:request");
    assert_eq!(item.category, "dependencies");
    assert_eq!(item.severity, Severity::Medium);
    assert_eq!(item.title, "request is deprecated (npm)");
    assert!(item
        .description
        .contains("request has been deprecated, see issue #3142"));
}

#[test]
fn deprecated_work_item_has_fallback_copy_when_message_is_missing() {
    let pkg = PackageUpdate {
        name: "old-pkg".into(),
        current_version: "1.0.0".into(),
        latest_version: "1.0.0".into(),
        ecosystem: Ecosystem::Npm,
        is_deprecated: true,
        ..Default::default()
    };

    let item = build_deprecated_work_item(7, "https://example.com".into(), &pkg, 1_000);

    assert_eq!(
        item.description,
        "The maintainer marked old-pkg deprecated. Plan a replacement."
    );
}

#[test]
fn ci_failures_emit_project_level_alerts() {
    let ci = CiFailure {
        workflow_name: "CI".into(),
        run_id: 123,
        conclusion: "failure".into(),
        html_url: "https://github.com/acme/site/actions/runs/123".into(),
        commit_sha: "abcdef12345".into(),
        completed_at: "2026-05-05T12:00:00Z".into(),
    };

    let alert = build_ci_failure_alert(7, &ci, 1_000);

    assert_eq!(alert.source, "github");
    assert_eq!(alert.severity, "warn");
    assert_eq!(alert.env_url, None);
    assert!(alert.alert_id.contains("ci-failure:CI:123"));
}
