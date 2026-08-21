use super::super::super::*;

#[test]
fn detects_unindexed_lookup_columns_in_local_sqlite_db() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }
            "#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE Membership (id TEXT PRIMARY KEY, user_id TEXT, workspace_id TEXT)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-sqlite-unindexed-lookups:")));
}

#[test]
fn detects_local_sqlite_lookup_columns_without_foreign_keys() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }
            "#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE User (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute("CREATE TABLE Workspace (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute(
        "CREATE TABLE Membership (id TEXT PRIMARY KEY, userId TEXT, workspaceId TEXT)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-sqlite-missing-foreign-keys:")));
}

#[test]
fn skips_local_sqlite_foreign_key_issue_when_constraints_exist() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }
            "#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE User (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute("CREATE TABLE Workspace (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute(
        r#"
                CREATE TABLE Membership (
                  id TEXT PRIMARY KEY,
                  userId TEXT,
                  workspaceId TEXT,
                  FOREIGN KEY(userId) REFERENCES User(id),
                  FOREIGN KEY(workspaceId) REFERENCES Workspace(id)
                )
            "#,
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-sqlite-missing-foreign-keys:")));
}
