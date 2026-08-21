use super::super::super::*;

#[test]
fn detects_local_prisma_sqlite_missing_migration_history_table() {
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
            "#,
    );
    write_file(
        temp.path(),
        "prisma/migrations/20260409010101_init/migration.sql",
        "CREATE TABLE User (id TEXT PRIMARY KEY);\n",
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE User (id TEXT PRIMARY KEY)", [])
        .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report.issues.iter().any(|issue| issue
        .id
        .starts_with("local-prisma-migration-history-missing:")));
}

#[test]
fn detects_local_prisma_sqlite_missing_applied_migrations() {
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
    write_file(
        temp.path(),
        "prisma/migrations/20260409010101_init/migration.sql",
        "CREATE TABLE User (id TEXT PRIMARY KEY);\n",
    );
    write_file(
        temp.path(),
        "prisma/migrations/20260409010202_membership/migration.sql",
        "CREATE TABLE Membership (id TEXT PRIMARY KEY, userId TEXT);\n",
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE User (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute(
        "CREATE TABLE Membership (id TEXT PRIMARY KEY, userId TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE _prisma_migrations (migration_name TEXT PRIMARY KEY)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO _prisma_migrations (migration_name) VALUES (?1)",
        ["20260409010101_init"],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-prisma-migration-drift:")));
}

#[test]
fn skips_local_prisma_migration_issue_when_history_is_aligned() {
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
            "#,
    );
    write_file(
        temp.path(),
        "prisma/migrations/20260409010101_init/migration.sql",
        "CREATE TABLE User (id TEXT PRIMARY KEY);\n",
    );
    write_file(temp.path(), ".env.local", "DATABASE_URL=file:./dev.db\n");

    let db_path = temp.path().join("dev.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("CREATE TABLE User (id TEXT PRIMARY KEY)", [])
        .unwrap();
    conn.execute(
        "CREATE TABLE _prisma_migrations (migration_name TEXT PRIMARY KEY)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO _prisma_migrations (migration_name) VALUES (?1)",
        ["20260409010101_init"],
    )
    .unwrap();

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("local-prisma-migration-")));
}
