use super::super::*;

#[test]
fn detects_lookup_heavy_schema_without_index_hints() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "postgresql"
                  url      = env("DATABASE_URL")
                }

                model Membership {
                  id          String @id
                  userId      String
                  workspaceId String
                  role        String
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("db-index-hints-missing:"))
        .expect("expected lookup-field index review");
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("query workload"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("EXPLAIN"));
    assert!(!issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .starts_with("Add explicit index"));
}

#[test]
fn skips_index_hint_issue_when_schema_has_explicit_indexes() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "postgresql"
                  url      = env("DATABASE_URL")
                }

                model Membership {
                  id          String @id
                  userId      String
                  workspaceId String
                  role        String

                  @@index([userId])
                  @@index([workspaceId])
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("db-index-hints-missing:")));
}

#[test]
fn detects_source_prisma_join_model_missing_composite_unique() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model Membership {
                  id          String @id
                  userId      String
                  workspaceId String
                  role        String
                  user        User      @relation(fields: [userId], references: [id], onDelete: Cascade)
                  workspace   Workspace @relation(fields: [workspaceId], references: [id], onDelete: Cascade)
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue
                .id
                .starts_with("schema-join-missing-composite-unique:")
        })
        .expect("composite uniqueness review issue");
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("may allow"));
    assert!(issue.description.contains("domain expects"));
    assert!(!issue.description.contains("checked-in"));
}

#[test]
fn detects_source_prisma_join_model_with_nullable_relations() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model Membership {
                  id          String @id
                  userId      String?
                  workspaceId String
                  role        String
                  user        User?     @relation(fields: [userId], references: [id])
                  workspace   Workspace @relation(fields: [workspaceId], references: [id])
                  @@unique([userId, workspaceId])
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-join-nullable-relations:")));
}

#[test]
fn skips_source_prisma_join_issue_when_schema_is_strict() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model Membership {
                  id          String @id
                  userId      String
                  workspaceId String
                  role        String
                  user        User      @relation(fields: [userId], references: [id], onDelete: Cascade)
                  workspace   Workspace @relation(fields: [workspaceId], references: [id], onDelete: Cascade)
                  @@unique([userId, workspaceId])
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-join-")));
}

#[test]
fn detects_source_prisma_join_model_missing_delete_intent() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model Membership {
                  id          String @id
                  userId      String
                  workspaceId String
                  role        String
                  user        User      @relation(fields: [userId], references: [id])
                  workspace   Workspace @relation(fields: [workspaceId], references: [id])
                  @@unique([userId, workspaceId])
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-join-missing-delete-intent:")));
}

#[test]
fn skips_source_prisma_join_delete_intent_when_explicit() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model Membership {
                  id          String @id
                  userId      String
                  workspaceId String
                  role        String
                  user        User      @relation(fields: [userId], references: [id], onDelete: Cascade)
                  workspace   Workspace @relation(fields: [workspaceId], references: [id], onDelete: Restrict)
                  @@unique([userId, workspaceId])
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-join-missing-delete-intent:")));
}

#[test]
fn detects_source_prisma_relation_missing_index() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model Post {
                  id       String @id
                  authorId String
                  title    String
                  author   User   @relation(fields: [authorId], references: [id], onDelete: Cascade)
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-relation-missing-index:")));
}

#[test]
fn skips_source_prisma_relation_missing_index_when_index_exists() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
                datasource db {
                  provider = "sqlite"
                  url      = env("DATABASE_URL")
                }

                model Post {
                  id       String @id
                  authorId String
                  title    String
                  author   User   @relation(fields: [authorId], references: [id], onDelete: Cascade)

                  @@index([authorId])
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-relation-missing-index:")));
}

#[test]
fn detects_source_drizzle_join_table_schema_issues() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/db/schema.ts",
        r#"
                import { sqliteTable, text } from "drizzle-orm/sqlite-core";

                export const memberships = sqliteTable("memberships", {
                  id: text("id").primaryKey(),
                  userId: text("user_id"),
                  workspaceId: text("workspace_id"),
                  role: text("role").notNull(),
                });
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report.issues.iter().any(|issue| issue
        .id
        .starts_with("schema-join-missing-composite-unique:")));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-join-nullable-relations:")));
}

#[test]
fn detects_source_drizzle_join_table_missing_delete_intent() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/db/schema.ts",
        r#"
                import { sqliteTable, text, uniqueIndex } from "drizzle-orm/sqlite-core";

                export const memberships = sqliteTable("memberships", {
                  id: text("id").primaryKey(),
                  userId: text("user_id").notNull().references(() => users.id),
                  workspaceId: text("workspace_id").notNull().references(() => workspaces.id),
                  role: text("role").notNull(),
                }, (table) => ({
                  membershipUnique: uniqueIndex("memberships_user_workspace_unique").on(
                    table.userId,
                    table.workspaceId,
                  ),
                }));
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-join-missing-delete-intent:")));
}

#[test]
fn skips_source_drizzle_join_issue_when_schema_is_strict() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/db/schema.ts",
        r#"
                import { sqliteTable, text, uniqueIndex } from "drizzle-orm/sqlite-core";

                export const memberships = sqliteTable("memberships", {
                  id: text("id").primaryKey(),
                  userId: text("user_id").notNull().references(() => users.id, { onDelete: "cascade" }),
                  workspaceId: text("workspace_id").notNull().references(() => workspaces.id, { onDelete: "restrict" }),
                  role: text("role").notNull(),
                }, (table) => ({
                  membershipUnique: uniqueIndex("memberships_user_workspace_unique").on(
                    table.userId,
                    table.workspaceId,
                  ),
                }));
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-join-")));
}

#[test]
fn detects_source_drizzle_relation_missing_index() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/db/schema.ts",
        r#"
                import { sqliteTable, text } from "drizzle-orm/sqlite-core";

                export const posts = sqliteTable("posts", {
                  id: text("id").primaryKey(),
                  authorId: text("author_id").notNull().references(() => users.id, { onDelete: "cascade" }),
                  title: text("title").notNull(),
                });
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-relation-missing-index:")));
}

#[test]
fn skips_source_drizzle_relation_missing_index_when_index_exists() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/db/schema.ts",
        r#"
                import { index, sqliteTable, text } from "drizzle-orm/sqlite-core";

                export const posts = sqliteTable("posts", {
                  id: text("id").primaryKey(),
                  authorId: text("author_id").notNull().references(() => users.id, { onDelete: "cascade" }),
                  title: text("title").notNull(),
                }, (table) => ({
                  authorIdx: index("posts_author_idx").on(table.authorId),
                }));
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("schema-relation-missing-index:")));
}
