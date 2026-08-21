use super::super::super::*;

#[test]
fn default_code_scan_does_not_open_discovered_local_databases() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{"name":"privacy-boundary"}"#,
    );
    write_file(
        temp.path(),
        "schema.prisma",
        r#"
            datasource db {
              provider = "sqlite"
              url      = env("DATABASE_URL")
            }

            model User {
              id String @id
            }
        "#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");
    let conn = rusqlite::Connection::open(temp.path().join("dev.db")).unwrap();
    conn.execute("CREATE TABLE posts (id TEXT PRIMARY KEY)", [])
        .unwrap();
    drop(conn);

    let report = audit_project(temp.path()).expect("static Code Scan");

    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("local-sqlite-")),
        "local database findings require the explicit inspection option"
    );
}

#[test]
fn detects_local_sqlite_schema_drift_against_checked_in_schema() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model User {
                  id String @id
                }

                model Membership {
                  id     String @id
                  userId String
                }
            "#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE User (id TEXT PRIMARY KEY)", [])
        .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-sqlite-schema-drift:")));
}

#[test]
fn detects_local_sqlite_column_drift_against_expected_schema_columns() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model User {
                  id    String @id
                  email String
                  slug  String
                }
            "#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE User (id TEXT PRIMARY KEY, email TEXT)", [])
        .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-sqlite-column-drift:")));
}

#[test]
fn skips_column_drift_for_prisma_relation_fields() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model User {
                  id    String @id
                  posts Post[]
                }

                model Post {
                  id       String @id
                  title    String
                  author   User   @relation(fields: [authorId], references: [id])
                  authorId String
                }
            "#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE User (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute(
        "CREATE TABLE Post (id TEXT PRIMARY KEY, title TEXT, authorId TEXT)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("local-sqlite-column-drift:")),
        "relation fields must not drive column drift, got: {:?}",
        report
            .issues
            .iter()
            .map(|i| i.id.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn skips_local_sqlite_column_drift_when_columns_are_aligned() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model User {
                  id    String @id
                  email String
                  slug  String
                }
            "#,
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE User (id TEXT PRIMARY KEY, email TEXT, slug TEXT)",
        [],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-sqlite-column-drift:")));
}
