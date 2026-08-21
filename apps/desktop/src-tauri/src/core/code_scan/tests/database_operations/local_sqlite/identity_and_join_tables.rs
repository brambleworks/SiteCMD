use super::super::super::*;

#[test]
fn detects_local_sqlite_identity_columns_without_unique_constraints() {
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
        "CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT, slug TEXT)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report.issues.iter().any(|issue| issue
        .id
        .starts_with("local-sqlite-missing-unique-constraints:")));
}

#[test]
fn skips_local_sqlite_identity_issue_when_unique_constraints_exist() {
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
        "CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT UNIQUE, slug TEXT UNIQUE)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(!report.issues.iter().any(|issue| issue
        .id
        .starts_with("local-sqlite-missing-unique-constraints:")));
}

#[test]
fn detects_local_sqlite_join_table_without_composite_unique() {
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
                  role TEXT,
                  FOREIGN KEY(userId) REFERENCES User(id),
                  FOREIGN KEY(workspaceId) REFERENCES Workspace(id)
                )
            "#,
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report.issues.iter().any(|issue| issue
        .id
        .starts_with("local-sqlite-missing-composite-unique:")));
}

#[test]
fn skips_local_sqlite_join_table_when_composite_unique_exists() {
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
                  role TEXT,
                  FOREIGN KEY(userId) REFERENCES User(id),
                  FOREIGN KEY(workspaceId) REFERENCES Workspace(id)
                )
            "#,
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE UNIQUE INDEX membership_user_workspace_unique ON Membership(userId, workspaceId)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(!report.issues.iter().any(|issue| issue
        .id
        .starts_with("local-sqlite-missing-composite-unique:")));
}

#[test]
fn detects_local_sqlite_join_table_with_nullable_relations() {
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
                  workspaceId TEXT NOT NULL,
                  role TEXT,
                  FOREIGN KEY(userId) REFERENCES User(id),
                  FOREIGN KEY(workspaceId) REFERENCES Workspace(id)
                )
            "#,
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE UNIQUE INDEX membership_user_workspace_unique ON Membership(userId, workspaceId)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-sqlite-nullable-relations:")));
}

#[test]
fn skips_local_sqlite_join_table_when_relations_are_not_null() {
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
                  userId TEXT NOT NULL,
                  workspaceId TEXT NOT NULL,
                  role TEXT,
                  FOREIGN KEY(userId) REFERENCES User(id),
                  FOREIGN KEY(workspaceId) REFERENCES Workspace(id)
                )
            "#,
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE UNIQUE INDEX membership_user_workspace_unique ON Membership(userId, workspaceId)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-sqlite-nullable-relations:")));
}
