use super::super::{postgres::POSTGRES_TEXT, sqlite::SQLITE_TEXT};
use super::*;
use std::path::PathBuf;

fn table(name: &str, columns: &[&str]) -> LocalDbTableSnapshot {
    LocalDbTableSnapshot {
        name: name.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        non_null_columns: HashSet::new(),
        indexed_columns: HashSet::new(),
        unique_indexed_columns: HashSet::new(),
        unique_index_groups: Vec::new(),
        foreign_key_columns: HashSet::new(),
    }
}

fn sqlite_snapshot(tables: Vec<LocalDbTableSnapshot>) -> LocalSqliteSnapshot {
    LocalSqliteSnapshot {
        absolute_path: PathBuf::from("/project/prisma/dev.db"),
        relative_path: "prisma/dev.db".to_string(),
        tables,
        has_prisma_migrations_table: false,
        applied_prisma_migrations: HashSet::new(),
        has_drizzle_migrations_table: false,
        applied_drizzle_migration_count: 0,
    }
}

fn postgres_snapshot(tables: Vec<LocalDbTableSnapshot>) -> LocalPostgresSnapshot {
    LocalPostgresSnapshot {
        absolute_path: PathBuf::from("/project/.env.local"),
        relative_path: ".env.local".to_string(),
        database_name: Some("app".to_string()),
        host: Some("localhost".to_string()),
        tables,
        has_prisma_migrations_table: false,
        applied_prisma_migrations: HashSet::new(),
        has_drizzle_migrations_table: false,
        applied_drizzle_migration_count: 0,
    }
}

fn sqlite_engine() -> EngineDescriptor {
    EngineDescriptor {
        text: &SQLITE_TEXT,
        database_label: "the local SQLite database".to_string(),
        unmigrated_subject: "the SQLite database".to_string(),
    }
}

fn postgres_engine() -> EngineDescriptor {
    let database_label = "local Postgres database `app` on `localhost`".to_string();
    EngineDescriptor {
        text: &POSTGRES_TEXT,
        unmigrated_subject: database_label.clone(),
        database_label,
    }
}

fn find<'a>(issues: &'a [CodeIssue], id_prefix: &str) -> &'a CodeIssue {
    issues
        .iter()
        .find(|issue| issue.id.starts_with(id_prefix))
        .unwrap_or_else(|| panic!("expected an issue starting with `{id_prefix}`"))
}

// Keep migration and schema-drift copy consistent across database engines.

#[test]
fn sqlite_prisma_history_missing_copy_stays_within_observed_scope() {
    let mut issues = Vec::new();
    let snapshot = sqlite_snapshot(vec![table("User", &["id"])]);
    collect_migration_issues(
        &mut issues,
        &[],
        &snapshot,
        &sqlite_engine(),
        &["202601_init".to_string()],
        &[],
    );

    let issue = find(&issues, "local-prisma-migration-history-missing:");
    assert_eq!(
        issue.id,
        "local-prisma-migration-history-missing:prisma/dev.db"
    );
    assert_eq!(issue.category, "operations");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert_eq!(
        issue.title,
        "Local SQLite database is missing Prisma migration history"
    );
    assert!(issue.description.contains("scanned project"));
    assert!(issue.description.contains("may have been"));
    assert!(issue
        .evidence
        .as_deref()
        .unwrap_or_default()
        .contains("Scanned schema artifacts include Prisma migration 202601_init"));
    let user_copy = format!(
        "{} {} {} {} {}",
        issue.title,
        issue.description,
        issue.why_now.as_deref().unwrap_or_default(),
        issue.likely_fix.as_deref().unwrap_or_default(),
        issue.verify_hint.as_deref().unwrap_or_default()
    );
    assert!(!user_copy.contains("checked-in"));
    assert!(!user_copy.contains("actually shipping"));
    // Schema-snapshot finding against a binary.db file -> no source anchor.
    assert_eq!(issue.line, None);
    assert_eq!(issue.source_excerpt, None);
}

#[test]
fn postgres_prisma_history_missing_copy_stays_within_observed_scope() {
    let mut issues = Vec::new();
    let snapshot = postgres_snapshot(vec![table("User", &["id"])]);
    collect_migration_issues(
        &mut issues,
        &[],
        &snapshot,
        &postgres_engine(),
        &["202601_init".to_string()],
        &[],
    );

    let issue = find(&issues, "local-postgres-prisma-migration-history-missing:");
    assert_eq!(
        issue.id,
        "local-postgres-prisma-migration-history-missing:.env.local"
    );
    assert_eq!(
        issue.title,
        "Local Postgres database is missing Prisma migration history"
    );
    assert!(issue.description.contains("scanned project"));
    assert!(issue.description.contains("may have been"));
    assert!(issue
        .evidence
        .as_deref()
        .unwrap_or_default()
        .contains("Scanned schema artifacts include Prisma migration 202601_init"));
    let user_copy = format!(
        "{} {} {} {} {}",
        issue.title,
        issue.description,
        issue.why_now.as_deref().unwrap_or_default(),
        issue.likely_fix.as_deref().unwrap_or_default(),
        issue.verify_hint.as_deref().unwrap_or_default()
    );
    assert!(!user_copy.contains("checked-in"));
    assert!(!user_copy.contains("actually shipping"));
    assert_eq!(issue.line, None);
    assert_eq!(issue.source_excerpt, None);
}

#[test]
fn unmigrated_evidence_subject_differs_per_engine() {
    let mut sqlite_issues = Vec::new();
    collect_schema_drift_issues(
        &mut sqlite_issues,
        &[],
        &sqlite_snapshot(vec![]),
        &sqlite_engine(),
        &HashSet::new(),
        &["user".to_string()],
        &HashMap::new(),
    );
    let sqlite_issue = find(&sqlite_issues, "local-sqlite-unmigrated:");
    assert_eq!(sqlite_issue.severity, Severity::Medium);
    assert_eq!(
        sqlite_issue.evidence.as_deref(),
        Some("Expected local tables like user from schema/migration files, but the SQLite database currently exposes no user tables.")
    );

    let mut postgres_issues = Vec::new();
    collect_schema_drift_issues(
        &mut postgres_issues,
        &[],
        &postgres_snapshot(vec![]),
        &postgres_engine(),
        &HashSet::new(),
        &["user".to_string()],
        &HashMap::new(),
    );
    let postgres_issue = find(&postgres_issues, "local-postgres-unmigrated:");
    assert_eq!(
        postgres_issue.evidence.as_deref(),
        Some("Expected local tables like user from schema/migration files, but local Postgres database `app` on `localhost` currently exposes no user tables.")
    );
}

#[test]
fn postgres_prisma_migration_drift_is_flagged_with_missing_migrations() {
    let mut issues = Vec::new();
    let mut snapshot = postgres_snapshot(vec![table("User", &["id"])]);
    snapshot.has_prisma_migrations_table = true;
    snapshot
        .applied_prisma_migrations
        .insert("202601_init".to_string());
    collect_migration_issues(
        &mut issues,
        &[],
        &snapshot,
        &postgres_engine(),
        &["202601_init".to_string(), "202602_add_users".to_string()],
        &[],
    );

    let issue = find(&issues, "local-postgres-prisma-migration-drift:");
    // One missing migration stays Medium; two or more escalate to High.
    assert_eq!(issue.severity, Severity::Medium);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("202602_add_users"),
        "evidence must name the unapplied migration: {evidence}"
    );
}

#[test]
fn postgres_drizzle_history_missing_is_flagged() {
    let mut issues = Vec::new();
    let snapshot = postgres_snapshot(vec![table("User", &["id"])]);
    collect_migration_issues(
        &mut issues,
        &[],
        &snapshot,
        &postgres_engine(),
        &[],
        &["0000_init".to_string()],
    );

    let issue = find(&issues, "local-postgres-drizzle-migration-history-missing:");
    assert_eq!(issue.severity, Severity::Medium);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("__drizzle_migrations"),
        "evidence must name the missing history table: {evidence}"
    );
}

#[test]
fn postgres_drizzle_migration_drift_stays_medium_for_local_state() {
    let mut issues = Vec::new();
    let mut snapshot = postgres_snapshot(vec![table("User", &["id"])]);
    snapshot.has_drizzle_migrations_table = true;
    snapshot.applied_drizzle_migration_count = 1;
    collect_migration_issues(
        &mut issues,
        &[],
        &snapshot,
        &postgres_engine(),
        &[],
        &[
            "0000_init".to_string(),
            "0001_users".to_string(),
            "0002_orders".to_string(),
        ],
    );

    let issue = find(&issues, "local-postgres-drizzle-migration-drift:");
    assert_eq!(issue.severity, Severity::Medium);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("only records 1 applied"),
        "evidence must carry the applied count: {evidence}"
    );
}

#[test]
fn postgres_schema_drift_is_flagged_for_missing_expected_table() {
    let mut issues = Vec::new();
    let snapshot = postgres_snapshot(vec![table("user", &["id"])]);
    let actual_tables: HashSet<String> = ["user".to_string()].into_iter().collect();
    collect_schema_drift_issues(
        &mut issues,
        &[],
        &snapshot,
        &postgres_engine(),
        &actual_tables,
        &["user".to_string(), "orders".to_string()],
        &HashMap::new(),
    );

    let issue = find(&issues, "local-postgres-schema-drift:");
    assert_eq!(issue.severity, Severity::Medium);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("missing orders"),
        "evidence must name the missing table: {evidence}"
    );
}

#[test]
fn postgres_column_drift_is_flagged_for_missing_expected_column() {
    let mut issues = Vec::new();
    let snapshot = postgres_snapshot(vec![table("User", &["id"])]);
    let actual_tables: HashSet<String> = ["user".to_string()].into_iter().collect();
    let mut expected_columns = HashMap::new();
    expected_columns.insert(
        "user".to_string(),
        ["email".to_string()].into_iter().collect::<HashSet<_>>(),
    );
    collect_schema_drift_issues(
        &mut issues,
        &[],
        &snapshot,
        &postgres_engine(),
        &actual_tables,
        &["user".to_string()],
        &expected_columns,
    );

    let issue = find(&issues, "local-postgres-column-drift:");
    assert_eq!(issue.severity, Severity::Medium);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("User missing email"),
        "evidence must name the drifted column: {evidence}"
    );
}

#[test]
fn postgres_unindexed_lookups_are_flagged() {
    let mut issues = Vec::new();
    let snapshot = postgres_snapshot(vec![table("posts", &["user_id", "org_id"])]);
    collect_table_integrity_issues(
        &mut issues,
        &[],
        &snapshot,
        &postgres_engine(),
        &HashSet::new(),
    );

    let issue = find(&issues, "local-postgres-unindexed-lookups:");
    assert_eq!(issue.severity, Severity::Medium);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("posts.user_id"),
        "evidence must name the unindexed column: {evidence}"
    );
}

#[test]
fn postgres_missing_unique_constraints_are_flagged_for_identity_columns() {
    let mut issues = Vec::new();
    let snapshot = postgres_snapshot(vec![table("users", &["email"])]);
    collect_table_integrity_issues(
        &mut issues,
        &[],
        &snapshot,
        &postgres_engine(),
        &HashSet::new(),
    );

    let issue = find(&issues, "local-postgres-missing-unique-constraints:");
    // A single identity column without unique coverage stays Low.
    assert_eq!(issue.severity, Severity::Low);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("users.email"),
        "evidence must name the unconstrained identity column: {evidence}"
    );
}

#[test]
fn table_integrity_unindexed_lookups_copy_is_preserved() {
    let mut issues = Vec::new();
    let snapshot = sqlite_snapshot(vec![table("posts", &["user_id", "org_id"])]);
    collect_table_integrity_issues(
        &mut issues,
        &[],
        &snapshot,
        &sqlite_engine(),
        &HashSet::new(),
    );

    let issue = find(&issues, "local-sqlite-unindexed-lookups:");
    assert_eq!(issue.category, "architecture");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.title,
        "Local SQLite schema has lookup-heavy columns without indexes"
    );
    assert_eq!(
        issue.evidence.as_deref(),
        Some("Detected lookup-style columns without local SQLite index coverage: posts.user_id, posts.org_id.")
    );
}
