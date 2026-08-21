use super::*;

use engine::{
    collect_migration_issues, collect_schema_drift_issues, collect_table_integrity_issues,
    EngineDescriptor, EngineText, IssueText,
};

/// SQLite user-facing copy. The check shapes live in `engine.rs`.
pub(super) static SQLITE_TEXT: EngineText = EngineText {
    name: "SQLite",
    slug: "sqlite",
    migration_slug: "",
    prisma_history_missing: IssueText {
        title: "Local SQLite database is missing Prisma migration history",
        description: "The scanned project contains Prisma migration artifacts, and the configured local SQLite database has application tables but no `_prisma_migrations` history table. It may have been created with schema push, manual SQL, a different migration system, or a history table that was removed; the scan cannot determine which.",
        why_now: "Without matching migration history, the local database cannot reliably demonstrate that the project's Prisma migration sequence applies cleanly.",
        likely_fix: "Confirm the intended local schema workflow. If Prisma migrations are authoritative, preserve any needed local data and then apply them or recreate the disposable local database through that workflow. If another workflow is intentional, document it and mark this finding not applicable.",
        verify_hint: "Run the documented local schema workflow, then re-run Code Scan and compare `_prisma_migrations` with the migration artifacts the current branch is meant to use.",
    },
    prisma_drift: IssueText {
        title: "Local SQLite Prisma history differs from scanned migrations",
        description: "The configured local SQLite database has Prisma migration history, but one or more migration directory names found in the scanned project are absent from that history. The database may be stale, or the artifacts may belong to another branch or a deliberately squashed history; deployment state was not inspected.",
        why_now: "A mismatch can make local tests and schema inspection unrepresentative of the current branch until the intended migration lineage is confirmed.",
        likely_fix: "Review the named migration differences against the current branch and the team's migration policy. Apply genuinely pending migrations to a disposable local copy, or recreate it from the intended history; do not apply artifacts from the wrong branch blindly.",
        verify_hint: "Re-run the documented Prisma status or migration command and Code Scan, then confirm the local history matches the migration lineage intended for this branch.",
    },
    drizzle_history_missing: IssueText {
        title: "Local SQLite database is missing Drizzle migration history",
        description: "The scanned project contains Drizzle migration artifacts, and the configured local SQLite database has application tables but no `__drizzle_migrations` history table. It may have been created with schema push, manual SQL, another migration system, or different Drizzle metadata configuration.",
        why_now: "Without the expected history metadata, the local database cannot demonstrate that the project's Drizzle migration sequence applies cleanly.",
        likely_fix: "Confirm the intended local schema workflow and Drizzle metadata configuration. If these migrations are authoritative, preserve any needed local data and then apply them or recreate the disposable local database through that workflow. Otherwise document the alternate workflow and mark this finding not applicable.",
        verify_hint: "Run the documented local Drizzle workflow, then re-run Code Scan and confirm the configured history table records the migration set intended for this branch.",
    },
    drizzle_drift: IssueText {
        title: "Local SQLite has fewer Drizzle history rows than scanned migrations",
        description: "The configured local SQLite database records fewer Drizzle migration rows than the number of migration artifacts found in the scanned project. This count mismatch may indicate pending migrations, but squashing, branch differences, or custom metadata can also explain it; the scan does not compare Drizzle migration identities.",
        why_now: "Until the count mismatch is explained, local tests and schema inspection may not represent the migration state intended for the current branch.",
        likely_fix: "Use the project's Drizzle status tooling and migration policy to identify whether migrations are actually pending. Apply confirmed pending migrations to a disposable local copy or recreate it from the intended lineage; do not infer order from the count alone.",
        verify_hint: "Run the documented Drizzle status and migration flow, then re-run Code Scan and confirm either the counts align or the intentional metadata difference is documented.",
    },
    unmigrated: IssueText {
        title: "Local SQLite database looks unmigrated relative to the project schema",
        description: "The scanned project contains schema or migration artifacts, but the configured local SQLite database exposes no application tables. The database may be new, pointed at the wrong file, or intentionally managed by another workflow.",
        why_now: "DB-aware findings cannot represent the application schema until the scanner is connected to the intended local database and that schema is present.",
        likely_fix: "Verify the local database path and run the project's documented schema or migration workflow if this file is meant to mirror the application. Keep it empty and mark the finding not applicable if that is intentional.",
        verify_hint: "Confirm the path targets the intended local database, run the documented setup flow, and re-run Code Scan to verify the expected application tables are visible.",
    },
    schema_drift: IssueText {
        title: "Local SQLite is missing tables inferred from scanned schema artifacts",
        description: "The configured local SQLite database does not expose one or more tables inferred from schema or migration artifacts in the scanned project. This may be local schema drift, but branch-specific, conditional, renamed, dropped, or parser-ambiguous artifacts can also produce the mismatch.",
        why_now: "An unexplained table mismatch can make local tests and DB-aware findings unrepresentative of the schema intended for the current branch.",
        likely_fix: "Review the named tables against the authoritative schema and migration lineage. If they should exist locally, apply the documented migrations or recreate a disposable local database; otherwise correct or retire the stale artifact or mark the inference not applicable.",
        verify_hint: "Inspect the authoritative schema and local database, then re-run the migration flow and Code Scan to confirm each named mismatch is resolved or intentionally explained.",
    },
    column_drift: IssueText {
        title: "Local SQLite is missing columns inferred from scanned schema artifacts",
        description: "The configured local SQLite database is missing one or more columns inferred from the scanned Prisma schema, SQL migrations, or Drizzle definitions. The mismatch may reflect stale local state, or it may come from branch-specific, renamed, dropped, conditional, or parser-ambiguous artifacts.",
        why_now: "If application code expects a genuinely missing column, local verification can fail or miss the behavior intended for the current branch.",
        likely_fix: "Review each named table and column against the authoritative schema lineage. Apply confirmed pending migrations to a disposable local database, or correct stale artifacts; do not add columns solely to silence a static inference.",
        verify_hint: "Inspect the local schema and authoritative migration state, exercise the affected model locally, and re-run Code Scan to confirm the mismatch is resolved or documented.",
    },
    unindexed_lookups: IssueText {
        title: "Local SQLite schema has lookup-heavy columns without indexes",
        description: "The configured local SQLite schema has multiple columns whose names resemble lookup or foreign-key fields without obvious index coverage. Names alone do not prove those columns are queried, selective, or worth indexing; workload and query plans were not inspected.",
        why_now: "Frequently filtered or joined columns can become slow as data grows, while unnecessary indexes add write, storage, and maintenance cost.",
        likely_fix: "Review real query paths and local `EXPLAIN QUERY PLAN` output. Add indexes only for confirmed filters, joins, or constraints where the expected read benefit justifies the write cost, and record them in the authoritative schema or migrations.",
        verify_hint: "Compare representative query plans and timings before and after any index change, then confirm writes and migrations still behave correctly; mark unused lookup-shaped fields not applicable.",
    },
    missing_foreign_keys: IssueText {
        title: "Local SQLite schema has relation-shaped columns without foreign key constraints",
        description: "The configured local SQLite schema has columns whose names suggest relations to known tables, but no matching foreign key was observed. The inference can be wrong for polymorphic links, external identifiers, denormalized data, or intentionally application-managed relations.",
        why_now: "A missing constraint can permit orphaned rows when the relation is real, while an inappropriate foreign key can reject valid lifecycle or cross-system data.",
        likely_fix: "Confirm each inferred relationship and its delete/update lifecycle. Add a foreign key through the authoritative schema or migrations only where the database should enforce it; otherwise document the application-level or external relationship.",
        verify_hint: "Ensure SQLite foreign-key enforcement is enabled for the application connection, test valid and invalid relation writes plus delete/update behavior, and mark intentional soft relations not applicable.",
    },
    missing_unique_constraints: IssueText {
        title: "Local SQLite schema has identity-style columns without unique constraints",
        description: "The configured local SQLite schema has columns with identity-like names such as email, slug, or external ID without obvious unique coverage. The names do not establish whether uniqueness is global, tenant-scoped, case-insensitive, conditional, or required at all.",
        why_now: "When the application relies on uniqueness that the database does not enforce, concurrent writes can create duplicates; the wrong constraint can also reject legitimate records.",
        likely_fix: "Confirm the domain rule and existing data first. Add the appropriate global, composite, partial, or normalized unique constraint through the authoritative schema only when that rule is real.",
        verify_hint: "Test duplicate and legitimately distinct cases, including tenant and case behavior, on a local copy and confirm the chosen constraint matches application semantics.",
    },
    missing_composite_unique: IssueText {
        title: "Local SQLite join-style tables are missing composite uniqueness",
        description: "The configured local SQLite schema has a join-like table with multiple relation-shaped columns and no composite unique constraint covering them. Duplicate pairs may be invalid, but temporal history, roles, versions, or other domain fields can make repeated pairs intentional.",
        why_now: "If one relation pair represents one logical membership or assignment, database uniqueness prevents race-created duplicates; otherwise a narrower constraint would be incorrect.",
        likely_fix: "Confirm the table's logical identity and existing duplicates. Add the correct composite or partial unique constraint only when repeated relation tuples are invalid, including any role, version, or lifecycle field that belongs in the key.",
        verify_hint: "Test both a duplicate tuple and every legitimately repeatable case on a local copy, then confirm the constraint matches the domain rather than only the column names.",
    },
    nullable_relations: IssueText {
        title: "Local SQLite join-style tables allow null relation columns",
        description: "The configured local SQLite schema has a join-like table with one or more nullable relation-shaped columns. Nulls may permit invalid partial links, but they can also represent a deliberate draft, invitation, migration, or polymorphic lifecycle state.",
        why_now: "A required relation should be enforced consistently, while changing an intentional nullable lifecycle can break valid records or migrations.",
        likely_fix: "Confirm the table lifecycle and inspect existing null rows. Make a relation `NOT NULL` through the authoritative schema only when every valid state requires it, with an explicit data migration for existing rows.",
        verify_hint: "Exercise creation and transition paths for both null and populated states on a local copy, then confirm the final nullability matches the documented lifecycle.",
    },
};

pub(super) fn collect_local_sqlite_issues(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    local_sqlite_snapshots: &[LocalSqliteSnapshot],
    expected_db_tables: &[String],
    expected_db_columns: &HashMap<String, HashSet<String>>,
    expected_prisma_migrations: &[String],
    expected_drizzle_migrations: &[String],
) {
    for snapshot in local_sqlite_snapshots {
        let actual_tables = snapshot
            .tables
            .iter()
            .map(|table| table.name.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let expected_db_table_set = expected_db_tables.iter().cloned().collect::<HashSet<_>>();
        let known_tables = actual_tables
            .union(&expected_db_table_set)
            .cloned()
            .collect::<HashSet<_>>();

        // The unmigrated evidence line uses SQLite's shorter subject.
        let engine = EngineDescriptor {
            text: &SQLITE_TEXT,
            database_label: "the local SQLite database".to_string(),
            unmigrated_subject: "the SQLite database".to_string(),
        };

        collect_migration_issues(
            issues,
            files,
            snapshot,
            &engine,
            expected_prisma_migrations,
            expected_drizzle_migrations,
        );
        collect_schema_drift_issues(
            issues,
            files,
            snapshot,
            &engine,
            &actual_tables,
            expected_db_tables,
            expected_db_columns,
        );
        collect_table_integrity_issues(issues, files, snapshot, &engine, &known_tables);
    }
}
