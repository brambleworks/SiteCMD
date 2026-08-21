use super::*;
use ::postgres::Config as PostgresConfig;

pub(in crate::core::code_scan) fn collect_local_postgres_snapshots(
    _root: &Path,
    env_files: &[EnvFileSnapshot],
) -> Result<Vec<LocalPostgresSnapshot>, String> {
    let mut snapshots = Vec::new();
    let mut seen_targets = HashSet::new();

    for env_file in env_files
        .iter()
        .filter(|file| is_local_dev_env_file(&file.relative_path))
    {
        for (key, value) in &env_file.entries {
            if !looks_like_database_url_key(key) || !looks_like_literal_database_url(value) {
                continue;
            }

            if crate::core::database_targets::is_mysql_database_target(value) {
                return Err(format!(
                    "Cannot inspect {} from {}: MySQL and MariaDB inspection is not supported. SiteCMD currently inspects only local SQLite and PostgreSQL targets.",
                    key, env_file.relative_path
                ));
            }

            let Ok(target) = validate_local_database_target(value) else {
                continue;
            };
            if target.kind != crate::core::database_targets::LocalDatabaseKind::Postgres {
                continue;
            }

            let normalized_target = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !seen_targets.insert(normalized_target.clone()) {
                continue;
            }

            if let Some(snapshot) = inspect_local_postgres_database(
                env_file,
                &normalized_target,
                target.database,
                target.host,
            ) {
                snapshots.push(snapshot);
            }
        }
    }

    Ok(snapshots)
}

fn inspect_local_postgres_database(
    env_file: &EnvFileSnapshot,
    connection_url: &str,
    database_name: Option<String>,
    host: Option<String>,
) -> Option<LocalPostgresSnapshot> {
    // The synchronous Postgres client creates its own runtime, so isolate it on
    // an OS thread rather than nesting that runtime inside Tokio.
    let connection_url = connection_url.to_string();
    let absolute_path = env_file.absolute_path.clone();
    let relative_path = env_file.relative_path.clone();
    std::thread::spawn(move || -> Option<LocalPostgresSnapshot> {
        let mut client = postgres_config(&connection_url).ok()?.connect(NoTls).ok()?;
        let _ = client.simple_query("SET statement_timeout = '3000ms'");
        let _ = client.simple_query("SET idle_in_transaction_session_timeout = '3000ms'");
        // Metadata inspection is intentionally incapable of writing even when
        // the supplied local role has write privileges. Every catalog query
        // below runs inside this server-enforced read-only transaction.
        let mut transaction = client.build_transaction().read_only(true).start().ok()?;

        let table_rows = transaction
            .query(
                "SELECT n.nspname, c.relname
             FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE c.relkind IN ('r', 'p')
               AND n.nspname NOT IN ('pg_catalog', 'information_schema')
               AND n.nspname NOT LIKE 'pg_toast%'
               AND n.nspname NOT LIKE 'pg_temp_%'
             ORDER BY n.nspname, c.relname",
                &[],
            )
            .ok()?;

        let migration_tables = table_rows
            .iter()
            .map(|row| (row.get::<_, String>(0), row.get::<_, String>(1)))
            .collect::<Vec<_>>();

        let has_prisma_migrations_table = migration_tables
            .iter()
            .any(|(_, table_name)| table_name.eq_ignore_ascii_case("_prisma_migrations"));
        let applied_prisma_migrations = if let Some((schema_name, _)) = migration_tables
            .iter()
            .find(|(_, table_name)| table_name.eq_ignore_ascii_case("_prisma_migrations"))
        {
            let rows = transaction
                .query(
                    &format!(
                        "SELECT migration_name FROM {}._prisma_migrations ORDER BY migration_name",
                        quote_postgres_identifier(schema_name)
                    ),
                    &[],
                )
                .ok()?;
            let mut migrations = HashSet::new();
            for row in rows {
                migrations.insert(row.try_get::<_, String>(0).ok()?.to_ascii_lowercase());
            }
            migrations
        } else {
            HashSet::new()
        };

        let has_drizzle_migrations_table = migration_tables
            .iter()
            .any(|(_, table_name)| table_name.eq_ignore_ascii_case("__drizzle_migrations"));
        let applied_drizzle_migration_count = if let Some((schema_name, _)) = migration_tables
            .iter()
            .find(|(_, table_name)| table_name.eq_ignore_ascii_case("__drizzle_migrations"))
        {
            let row = transaction
                .query_one(
                    &format!(
                        "SELECT COUNT(*) FROM {}.__drizzle_migrations",
                        quote_postgres_identifier(schema_name)
                    ),
                    &[],
                )
                .ok()?;
            row.try_get::<_, i64>(0).ok()?.max(0) as usize
        } else {
            0
        };

        let mut tables = Vec::new();
        for row in table_rows {
            let schema_name = row.get::<_, String>(0);
            let table_name = row.get::<_, String>(1);
            if table_name.eq_ignore_ascii_case("_prisma_migrations")
                || table_name.eq_ignore_ascii_case("__drizzle_migrations")
            {
                continue;
            }

            let column_rows = transaction
                .query(
                    "SELECT column_name, is_nullable = 'NO'
                 FROM information_schema.columns
                 WHERE table_schema = $1 AND table_name = $2
                 ORDER BY ordinal_position",
                    &[&schema_name, &table_name],
                )
                .ok()?;
            let mut columns = Vec::new();
            let mut non_null_columns = HashSet::new();
            for row in &column_rows {
                let name = row.try_get::<_, String>(0).ok()?;
                let non_null = row.try_get::<_, bool>(1).ok()?;
                if non_null {
                    non_null_columns.insert(name.to_ascii_lowercase());
                }
                columns.push(name);
            }

            let index_rows = transaction
                .query(
                    "SELECT i.relname, ix.indisunique, a.attname
                 FROM pg_class t
                 JOIN pg_namespace n ON n.oid = t.relnamespace
                 JOIN pg_index ix ON t.oid = ix.indrelid
                 JOIN pg_class i ON i.oid = ix.indexrelid
                 JOIN unnest(ix.indkey) WITH ORDINALITY AS key(attnum, ordinality) ON key.attnum > 0
                 JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = key.attnum
                 WHERE n.nspname = $1 AND t.relname = $2
                 ORDER BY i.relname, key.ordinality",
                    &[&schema_name, &table_name],
                )
                .ok()?;

            let mut indexed_columns = HashSet::new();
            let mut unique_indexed_columns = HashSet::new();
            let mut unique_index_groups_by_name: HashMap<String, HashSet<String>> = HashMap::new();
            for row in &index_rows {
                let index_name = row.try_get::<_, String>(0).ok()?;
                let is_unique = row.try_get::<_, bool>(1).ok()?;
                let column_name = row.try_get::<_, String>(2).ok()?;
                let normalized = column_name.to_ascii_lowercase();
                indexed_columns.insert(normalized.clone());
                if is_unique {
                    unique_indexed_columns.insert(normalized.clone());
                    unique_index_groups_by_name
                        .entry(index_name)
                        .or_default()
                        .insert(normalized);
                }
            }
            let unique_index_groups = unique_index_groups_by_name
                .into_values()
                .filter(|group| !group.is_empty())
                .collect::<Vec<_>>();

            let foreign_key_rows = transaction
                .query(
                    "SELECT kcu.column_name
                 FROM information_schema.table_constraints tc
                 JOIN information_schema.key_column_usage kcu
                   ON tc.constraint_name = kcu.constraint_name
                  AND tc.table_schema = kcu.table_schema
                 WHERE tc.constraint_type = 'FOREIGN KEY'
                   AND tc.table_schema = $1
                   AND tc.table_name = $2",
                    &[&schema_name, &table_name],
                )
                .ok()?;
            let mut foreign_key_columns = HashSet::new();
            for row in &foreign_key_rows {
                foreign_key_columns.insert(row.try_get::<_, String>(0).ok()?.to_ascii_lowercase());
            }

            tables.push(LocalDbTableSnapshot {
                name: table_name,
                columns,
                non_null_columns,
                indexed_columns,
                unique_indexed_columns,
                unique_index_groups,
                foreign_key_columns,
            });
        }

        Some(LocalPostgresSnapshot {
            absolute_path,
            relative_path,
            database_name,
            host,
            tables,
            has_prisma_migrations_table,
            applied_prisma_migrations,
            has_drizzle_migrations_table,
            applied_drizzle_migration_count,
        })
    })
    .join()
    .ok()
    .flatten()
}

fn postgres_config(connection_url: &str) -> Result<PostgresConfig, String> {
    let mut config =
        crate::core::database_targets::validated_local_postgres_config(connection_url)?;
    config.connect_timeout(crate::constants::CODE_SCAN_DATABASE_CONNECT_TIMEOUT);
    Ok(config)
}

fn quote_postgres_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::{inspect_local_postgres_database, postgres_config};
    use crate::constants::CODE_SCAN_DATABASE_CONNECT_TIMEOUT;
    use crate::core::code_scan::database_analysis::types::EnvFileSnapshot;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    #[test]
    fn postgres_inspection_applies_a_connection_deadline_before_connecting() {
        let config = postgres_config("postgres://localhost/sitecmd").expect("postgres config");

        assert_eq!(
            config.get_connect_timeout(),
            Some(&CODE_SCAN_DATABASE_CONNECT_TIMEOUT),
        );
    }

    #[test]
    fn live_inspection_does_not_panic_inside_a_tokio_runtime() {
        // The synchronous client must stay off the async runtime.
        let env_file = EnvFileSnapshot {
            absolute_path: PathBuf::from("/tmp/.env.local"),
            relative_path: ".env.local".to_string(),
            content: String::new(),
            keys: HashSet::new(),
            entries: HashMap::new(),
        };
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime.block_on(async {
            inspect_local_postgres_database(
                &env_file,
                "postgres://localhost:59999/none",
                None,
                None,
            )
        });
        assert!(result.is_none());
    }
}
