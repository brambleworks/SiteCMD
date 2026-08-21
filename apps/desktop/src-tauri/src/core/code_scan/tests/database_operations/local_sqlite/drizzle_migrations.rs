use super::super::super::*;

#[test]
fn detects_local_drizzle_sqlite_missing_migration_history_table() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "drizzle/0000_init.sql",
        "CREATE TABLE users (id TEXT PRIMARY KEY);\n",
    );
    write_file(
        temp.path(),
        "drizzle/meta/_journal.json",
        r#"{ "entries": [{ "tag": "0000_init" }] }"#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE users (id TEXT PRIMARY KEY)", [])
        .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report.issues.iter().any(|issue| issue
        .id
        .starts_with("local-drizzle-migration-history-missing:")));
}

#[test]
fn detects_local_drizzle_sqlite_missing_applied_migrations() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "drizzle/0000_init.sql",
        "CREATE TABLE users (id TEXT PRIMARY KEY);\n",
    );
    write_file(
        temp.path(),
        "drizzle/0001_membership.sql",
        "CREATE TABLE memberships (id TEXT PRIMARY KEY, user_id TEXT);\n",
    );
    write_file(
        temp.path(),
        "drizzle/meta/_journal.json",
        r#"{ "entries": [{ "tag": "0000_init" }, { "tag": "0001_membership" }] }"#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE users (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute(
        "CREATE TABLE memberships (id TEXT PRIMARY KEY, user_id TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE __drizzle_migrations (id INTEGER PRIMARY KEY, hash TEXT, created_at INTEGER)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO __drizzle_migrations (hash, created_at) VALUES (?1, ?2)",
        ["hash-0000", "1710000000"],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-drizzle-migration-drift:")));
}

#[test]
fn skips_local_drizzle_migration_issue_when_history_is_aligned() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "drizzle/0000_init.sql",
        "CREATE TABLE users (id TEXT PRIMARY KEY);\n",
    );
    write_file(
        temp.path(),
        "drizzle/meta/_journal.json",
        r#"{ "entries": [{ "tag": "0000_init" }] }"#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE users (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute(
        "CREATE TABLE __drizzle_migrations (id INTEGER PRIMARY KEY, hash TEXT, created_at INTEGER)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO __drizzle_migrations (hash, created_at) VALUES (?1, ?2)",
        ["hash-0000", "1710000000"],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-drizzle-migration-")));
}
