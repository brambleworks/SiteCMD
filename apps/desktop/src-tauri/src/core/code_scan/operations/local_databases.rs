use super::*;

mod engine;
mod postgres;
mod sqlite;

use postgres::collect_local_postgres_issues;
use sqlite::collect_local_sqlite_issues;

pub(super) fn collect_local_database_issues(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    local_sqlite_snapshots: &[LocalSqliteSnapshot],
    local_postgres_snapshots: &[LocalPostgresSnapshot],
    expected_db_tables: &[String],
    expected_db_columns: &HashMap<String, HashSet<String>>,
    expected_prisma_migrations: &[String],
    expected_drizzle_migrations: &[String],
) {
    collect_local_sqlite_issues(
        issues,
        files,
        local_sqlite_snapshots,
        expected_db_tables,
        expected_db_columns,
        expected_prisma_migrations,
        expected_drizzle_migrations,
    );
    collect_local_postgres_issues(
        issues,
        files,
        local_postgres_snapshots,
        expected_db_tables,
        expected_db_columns,
        expected_prisma_migrations,
        expected_drizzle_migrations,
    );
}
