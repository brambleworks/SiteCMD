use super::*;

#[test]
#[ignore = "requires SITECMD_POSTGRES_TEST_URL pointing at a localhost Postgres maintenance database"]
fn postgres_live_detects_missing_prisma_migration_history() {
    with_live_postgres_test_db("prisma_history_missing", |temp, _db_url, client| {
        write_file(
            temp.path(),
            "prisma/schema.prisma",
            r#"
                    datasource db {
                      provider = "postgresql"
                      url      = env("DATABASE_URL")
                    }

                    generator client {
                      provider = "prisma-client-js"
                    }

                    model User {
                      id    Int    @id @default(autoincrement())
                      email String @unique
                    }
                "#,
        );
        write_file(
            temp.path(),
            "prisma/migrations/202604090001_init/migration.sql",
            "CREATE TABLE \"User\" (id SERIAL PRIMARY KEY, email TEXT NOT NULL UNIQUE);",
        );

        client
            .batch_execute(
                r#"
                    CREATE TABLE "User" (
                      id SERIAL PRIMARY KEY,
                      email TEXT NOT NULL UNIQUE
                    );
                "#,
            )
            .expect("seed Postgres schema without prisma migration history");

        let report = audit_project_with_local_databases(temp.path()).expect("audit project");
        assert!(report.issues.iter().any(|issue| issue
            .id
            .starts_with("local-postgres-prisma-migration-history-missing:")));
    });
}

#[test]
#[ignore = "requires SITECMD_POSTGRES_TEST_URL pointing at a localhost Postgres maintenance database"]
fn postgres_live_detects_join_table_integrity_gaps() {
    with_live_postgres_test_db("join_integrity_gaps", |temp, _db_url, client| {
        // The auditor refuses folders with no project signals; the live
        // database is the only fixture this test actually exercises.
        write_file(
            temp.path(),
            "package.json",
            r#"{ "name": "join-integrity-fixture", "version": "1.0.0" }"#,
        );

        client
            .batch_execute(
                r#"
                    CREATE TABLE users (
                      id SERIAL PRIMARY KEY,
                      email TEXT NOT NULL
                    );

                    CREATE TABLE workspaces (
                      id SERIAL PRIMARY KEY,
                      slug TEXT NOT NULL
                    );

                    CREATE TABLE memberships (
                      id SERIAL PRIMARY KEY,
                      user_id INTEGER,
                      workspace_id INTEGER,
                      role TEXT
                    );
                "#,
            )
            .expect("seed Postgres join tables without integrity constraints");

        let report = audit_project_with_local_databases(temp.path()).expect("audit project");
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("local-postgres-missing-foreign-keys:")));
        assert!(report.issues.iter().any(|issue| issue
            .id
            .starts_with("local-postgres-missing-composite-unique:")));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("local-postgres-nullable-relations:")));
    });
}
