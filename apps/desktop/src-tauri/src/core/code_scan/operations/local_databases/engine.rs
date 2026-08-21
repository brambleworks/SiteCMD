use super::*;

/// Static user-facing copy for one local-database finding.
pub(super) struct IssueText {
    pub title: &'static str,
    pub description: &'static str,
    pub why_now: &'static str,
    pub likely_fix: &'static str,
    pub verify_hint: &'static str,
}

/// Engine-specific copy and identifier slugs.
pub(super) struct EngineText {
    /// Display name used in evidence.
    pub name: &'static str,
    /// ID slug for schema and integrity findings.
    pub slug: &'static str,
    /// Engine-specific slug for migration findings.
    pub migration_slug: &'static str,
    pub prisma_history_missing: IssueText,
    pub prisma_drift: IssueText,
    pub drizzle_history_missing: IssueText,
    pub drizzle_drift: IssueText,
    pub unmigrated: IssueText,
    pub schema_drift: IssueText,
    pub column_drift: IssueText,
    pub unindexed_lookups: IssueText,
    pub missing_foreign_keys: IssueText,
    pub missing_unique_constraints: IssueText,
    pub missing_composite_unique: IssueText,
    pub nullable_relations: IssueText,
}

/// One engine's view of a single snapshot: its static copy table plus the
/// runtime evidence subject phrases that vary per snapshot.
pub(super) struct EngineDescriptor {
    pub text: &'static EngineText,
    /// Subject noun phrase for most evidence sentences. SQLite is the constant
    /// "the local SQLite database"; Postgres is "local Postgres database
    /// `name` on `host`", built per snapshot from the connection metadata.
    pub database_label: String,
    /// Subject for the "unmigrated" evidence line. Postgres uses
    /// `database_label`; SQLite uses "the SQLite database".
    pub unmigrated_subject: String,
}

/// The snapshot fields the local-database checks read, exposed uniformly so one
/// set of check functions serves both the SQLite and Postgres engines.
pub(super) trait LocalDbSnapshot {
    fn relative_path(&self) -> &str;
    fn absolute_path_display(&self) -> String;
    fn tables(&self) -> &[LocalDbTableSnapshot];
    fn has_prisma_migrations_table(&self) -> bool;
    fn applied_prisma_migrations(&self) -> &HashSet<String>;
    fn has_drizzle_migrations_table(&self) -> bool;
    fn applied_drizzle_migration_count(&self) -> usize;
}

impl LocalDbSnapshot for LocalSqliteSnapshot {
    fn relative_path(&self) -> &str {
        &self.relative_path
    }
    fn absolute_path_display(&self) -> String {
        self.absolute_path.to_string_lossy().to_string()
    }
    fn tables(&self) -> &[LocalDbTableSnapshot] {
        &self.tables
    }
    fn has_prisma_migrations_table(&self) -> bool {
        self.has_prisma_migrations_table
    }
    fn applied_prisma_migrations(&self) -> &HashSet<String> {
        &self.applied_prisma_migrations
    }
    fn has_drizzle_migrations_table(&self) -> bool {
        self.has_drizzle_migrations_table
    }
    fn applied_drizzle_migration_count(&self) -> usize {
        self.applied_drizzle_migration_count
    }
}

impl LocalDbSnapshot for LocalPostgresSnapshot {
    fn relative_path(&self) -> &str {
        &self.relative_path
    }
    fn absolute_path_display(&self) -> String {
        self.absolute_path.to_string_lossy().to_string()
    }
    fn tables(&self) -> &[LocalDbTableSnapshot] {
        &self.tables
    }
    fn has_prisma_migrations_table(&self) -> bool {
        self.has_prisma_migrations_table
    }
    fn applied_prisma_migrations(&self) -> &HashSet<String> {
        &self.applied_prisma_migrations
    }
    fn has_drizzle_migrations_table(&self) -> bool {
        self.has_drizzle_migrations_table
    }
    fn applied_drizzle_migration_count(&self) -> usize {
        self.applied_drizzle_migration_count
    }
}

/// Anchor a database finding only when its artifact is readable source text.
/// Binary databases and unscanned environment files therefore have no line or
/// excerpt.
fn source_anchor(files: &[SourceFile], relative_path: &str) -> (Option<u32>, Option<String>) {
    let content = file_content_for_relative(files, relative_path);
    if content.is_empty() {
        return (None, None);
    }
    (Some(1), excerpt_for_line(content, Some(1)))
}

/// The fields of a finding that vary per check; every other `CodeIssue` field
/// is filled by [`push_issue`].
struct IssueDraft {
    id: String,
    category: &'static str,
    severity: Severity,
    text: &'static IssueText,
    evidence: String,
}

/// Assemble one `CodeIssue` from a draft, filling the fields that are identical
/// across every local-database finding (empty `check_id`, snapshot paths, the
/// shared line/excerpt policy, policy-graded confidence).
fn push_issue(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    snapshot: &impl LocalDbSnapshot,
    draft: IssueDraft,
) {
    let (line, source_excerpt) = source_anchor(files, snapshot.relative_path());
    // Inferred schema drift defaults to NeedsReview through the shared slug policy.
    let slug = draft.id.split(':').next().unwrap_or(draft.id.as_str());
    let (confidence, confidence_reason) = policy_confidence(slug);
    issues.push(CodeIssue {
        check_id: String::new(),
        id: draft.id.clone(),
        category: draft.category.into(),
        severity: draft.severity,
        title: draft.text.title.into(),
        description: draft.text.description.into(),
        relative_path: snapshot.relative_path().to_string(),
        absolute_path: snapshot.absolute_path_display(),
        line,
        source_excerpt,
        evidence: Some(redact_evidence(draft.evidence)),
        why_now: Some(draft.text.why_now.into()),
        likely_fix: Some(draft.text.likely_fix.into()),
        confidence,
        confidence_reason,
        verify_hint: Some(draft.text.verify_hint.into()),
    });
}

/// Migration-presence checks (Prisma + Drizzle history missing / behind).
pub(super) fn collect_migration_issues(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    snapshot: &impl LocalDbSnapshot,
    engine: &EngineDescriptor,
    expected_prisma_migrations: &[String],
    expected_drizzle_migrations: &[String],
) {
    let text = engine.text;
    let relative_path = snapshot.relative_path();

    if !expected_prisma_migrations.is_empty() && !snapshot.tables().is_empty() {
        if !snapshot.has_prisma_migrations_table() {
            push_issue(
                issues,
                files,
                snapshot,
                IssueDraft {
                    id: format!(
                        "local-{}prisma-migration-history-missing:{relative_path}",
                        text.migration_slug
                    ),
                    category: "operations",
                    // Medium: the copy frames the impact as local verification
                    // reliability, not a production exposure.
                    severity: Severity::Medium,
                    text: &text.prisma_history_missing,
                    evidence: format!(
                        "Scanned schema artifacts include Prisma migration {}, but {} has application tables and no `_prisma_migrations` history table.",
                        format_key_list(expected_prisma_migrations),
                        engine.database_label
                    ),
                },
            );
        } else {
            let missing_prisma_migrations = expected_prisma_migrations
                .iter()
                .filter(|migration| {
                    !snapshot
                        .applied_prisma_migrations()
                        .contains(migration.as_str())
                })
                .cloned()
                .collect::<Vec<_>>();

            if !missing_prisma_migrations.is_empty() {
                push_issue(
                    issues,
                    files,
                    snapshot,
                    IssueDraft {
                        id: format!(
                            "local-{}prisma-migration-drift:{relative_path}",
                            text.migration_slug
                        ),
                        category: "operations",
                        // This is local verification state, not evidence of a
                        // production outage or exposure. Count changes the
                        // amount of drift, not the impact class.
                        severity: Severity::Medium,
                        text: &text.prisma_drift,
                        evidence: format!(
                            "Scanned schema artifacts include Prisma migrations {}, but {} is missing {} in `_prisma_migrations`.",
                            format_key_list(expected_prisma_migrations),
                            engine.database_label,
                            format_key_list(&missing_prisma_migrations)
                        ),
                    },
                );
            }
        }
    }

    if !expected_drizzle_migrations.is_empty() && !snapshot.tables().is_empty() {
        if !snapshot.has_drizzle_migrations_table() {
            push_issue(
                issues,
                files,
                snapshot,
                IssueDraft {
                    id: format!(
                        "local-{}drizzle-migration-history-missing:{relative_path}",
                        text.migration_slug
                    ),
                    category: "operations",
                    // Medium: the copy frames the impact as local verification
                    // reliability, not a production exposure.
                    severity: Severity::Medium,
                    text: &text.drizzle_history_missing,
                    evidence: format!(
                        "Scanned schema artifacts include Drizzle migration {}, but {} has application tables and no `__drizzle_migrations` history table.",
                        format_key_list(expected_drizzle_migrations),
                        engine.database_label
                    ),
                },
            );
        } else if snapshot.applied_drizzle_migration_count() < expected_drizzle_migrations.len() {
            push_issue(
                issues,
                files,
                snapshot,
                IssueDraft {
                    id: format!(
                        "local-{}drizzle-migration-drift:{relative_path}",
                        text.migration_slug
                    ),
                    category: "operations",
                    severity: Severity::Medium,
                    text: &text.drizzle_drift,
                    evidence: format!(
                        "Scanned schema artifacts include {} Drizzle migrations ({}) but {} only records {} applied rows in `__drizzle_migrations`.",
                        expected_drizzle_migrations.len(),
                        format_key_list(expected_drizzle_migrations),
                        engine.database_label,
                        snapshot.applied_drizzle_migration_count()
                    ),
                },
            );
        }
    }
}

/// Schema-drift checks: unmigrated database, missing tables, missing columns.
pub(super) fn collect_schema_drift_issues(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    snapshot: &impl LocalDbSnapshot,
    engine: &EngineDescriptor,
    actual_tables: &HashSet<String>,
    expected_db_tables: &[String],
    expected_db_columns: &HashMap<String, HashSet<String>>,
) {
    let text = engine.text;
    let relative_path = snapshot.relative_path();

    if !expected_db_tables.is_empty() && snapshot.tables().is_empty() {
        push_issue(
            issues,
            files,
            snapshot,
            IssueDraft {
                id: format!("local-{}-unmigrated:{relative_path}", text.slug),
                category: "operations",
                // Missing local state affects verification reliability, not production security.
                severity: Severity::Medium,
                text: &text.unmigrated,
                evidence: format!(
                    "Expected local tables like {} from schema/migration files, but {} currently exposes no user tables.",
                    format_key_list(expected_db_tables),
                    engine.unmigrated_subject
                ),
            },
        );
    } else if !expected_db_tables.is_empty() {
        let missing_tables = expected_db_tables
            .iter()
            .filter(|table| !actual_tables.contains(table.as_str()))
            .cloned()
            .collect::<Vec<_>>();

        if !missing_tables.is_empty() {
            push_issue(
                issues,
                files,
                snapshot,
                IssueDraft {
                    id: format!("local-{}-schema-drift:{relative_path}", text.slug),
                    category: "operations",
                    severity: Severity::Medium,
                    text: &text.schema_drift,
                    evidence: format!(
                        "Schema artifacts suggest tables {}, but {} is missing {}.",
                        format_key_list(expected_db_tables),
                        engine.database_label,
                        format_key_list(&missing_tables)
                    ),
                },
            );
        }
    }

    let missing_columns_by_table = collect_missing_columns(snapshot.tables(), expected_db_columns);
    if !missing_columns_by_table.is_empty() {
        push_issue(
            issues,
            files,
            snapshot,
            IssueDraft {
                id: format!("local-{}-column-drift:{relative_path}", text.slug),
                category: "operations",
                severity: Severity::Medium,
                text: &text.column_drift,
                evidence: format!(
                    "Detected local {} column drift: {}.",
                    text.name,
                    missing_columns_by_table.join("; ")
                ),
            },
        );
    }
}

/// Columns the local schema expects but the actual tables are missing, one
/// entry per affected table (`"users missing email, slug"`).
fn collect_missing_columns(
    tables: &[LocalDbTableSnapshot],
    expected_db_columns: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    let mut missing_columns_by_table = Vec::new();
    for table in tables {
        let table_key = table.name.to_ascii_lowercase();
        let Some(expected_columns) = expected_db_columns.get(&table_key) else {
            continue;
        };
        if expected_columns.is_empty() {
            continue;
        }

        let actual_columns = table
            .columns
            .iter()
            .map(|column| column.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let missing_columns = expected_columns
            .iter()
            .filter(|column| !actual_columns.contains(column.as_str()))
            .cloned()
            .collect::<Vec<_>>();

        if !missing_columns.is_empty() {
            missing_columns_by_table.push(format!(
                "{} missing {}",
                table.name,
                format_key_list(&missing_columns)
            ));
        }
    }
    missing_columns_by_table
}

/// Aggregated table-integrity signals across one snapshot's tables.
struct IntegrityFindings {
    unindexed_lookup_columns: Vec<String>,
    unconstrained_lookup_columns: Vec<String>,
    non_unique_identity_columns: Vec<String>,
    missing_composite_unique_tables: Vec<String>,
    nullable_join_relation_columns: Vec<String>,
}

/// Collect table-integrity signals in one pass.
fn analyze_table_integrity(
    tables: &[LocalDbTableSnapshot],
    known_tables: &HashSet<String>,
) -> IntegrityFindings {
    let mut unindexed_lookup_columns = Vec::new();
    let mut unconstrained_lookup_columns = Vec::new();
    let mut non_unique_identity_columns = Vec::new();
    let mut missing_composite_unique_tables = Vec::new();
    let mut nullable_join_relation_columns = Vec::new();
    for table in tables {
        let mut lookup_columns_for_table = Vec::new();
        for column in &table.columns {
            let is_lookup = DB_LOOKUP_FIELD_PATTERNS
                .iter()
                .any(|pattern| pattern.is_match(column));
            let is_identity = DB_IDENTITY_FIELD_PATTERNS
                .iter()
                .any(|pattern| pattern.is_match(column));

            if is_identity
                && !table
                    .unique_indexed_columns
                    .contains(&column.to_ascii_lowercase())
            {
                non_unique_identity_columns.push(format!("{}.{}", table.name, column));
            }

            if !is_lookup {
                continue;
            }

            lookup_columns_for_table.push(column.to_ascii_lowercase());

            if !table.indexed_columns.contains(&column.to_ascii_lowercase()) {
                unindexed_lookup_columns.push(format!("{}.{}", table.name, column));
            }

            if !table
                .foreign_key_columns
                .contains(&column.to_ascii_lowercase())
            {
                let probable_targets = infer_relation_targets_from_column(column)
                    .into_iter()
                    .filter(|candidate| known_tables.contains(candidate))
                    .collect::<Vec<_>>();
                if !probable_targets.is_empty() {
                    unconstrained_lookup_columns.push(format!(
                        "{}.{} -> {}",
                        table.name,
                        column,
                        probable_targets.join(" | ")
                    ));
                }
            }
        }

        if lookup_columns_for_table.len() >= 2 {
            let non_lookup_columns = table
                .columns
                .iter()
                .filter(|column| {
                    !lookup_columns_for_table
                        .iter()
                        .any(|lookup| lookup == &column.to_ascii_lowercase())
                })
                .map(|column| column.to_ascii_lowercase())
                .collect::<Vec<_>>();
            let join_like = non_lookup_columns
                .iter()
                .all(|column| is_join_like_metadata_column(column));
            let has_covering_composite_unique = table.unique_index_groups.iter().any(|group| {
                lookup_columns_for_table
                    .iter()
                    .all(|column| group.contains(column.as_str()))
            });

            if join_like && !has_covering_composite_unique {
                missing_composite_unique_tables.push(format!(
                    "{} ({})",
                    table.name,
                    lookup_columns_for_table.join(", ")
                ));
            }

            let nullable_lookup_columns = lookup_columns_for_table
                .iter()
                .filter(|column| !table.non_null_columns.contains(column.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            if join_like && !nullable_lookup_columns.is_empty() {
                nullable_join_relation_columns.push(format!(
                    "{} ({})",
                    table.name,
                    nullable_lookup_columns.join(", ")
                ));
            }
        }
    }

    IntegrityFindings {
        unindexed_lookup_columns,
        unconstrained_lookup_columns,
        non_unique_identity_columns,
        missing_composite_unique_tables,
        nullable_join_relation_columns,
    }
}

/// Table-integrity checks: unindexed lookups, missing foreign keys, missing
/// unique/composite-unique constraints, and nullable join relations.
pub(super) fn collect_table_integrity_issues(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    snapshot: &impl LocalDbSnapshot,
    engine: &EngineDescriptor,
    known_tables: &HashSet<String>,
) {
    let text = engine.text;
    let relative_path = snapshot.relative_path();
    let findings = analyze_table_integrity(snapshot.tables(), known_tables);

    if findings.unindexed_lookup_columns.len() >= 2 {
        push_issue(
            issues,
            files,
            snapshot,
            IssueDraft {
                id: format!("local-{}-unindexed-lookups:{relative_path}", text.slug),
                category: "architecture",
                severity: Severity::Medium,
                text: &text.unindexed_lookups,
                evidence: format!(
                    "Detected lookup-style columns without local {} index coverage: {}.",
                    text.name,
                    format_key_list(&findings.unindexed_lookup_columns)
                ),
            },
        );
    }

    if !findings.unconstrained_lookup_columns.is_empty() {
        push_issue(
            issues,
            files,
            snapshot,
            IssueDraft {
                id: format!("local-{}-missing-foreign-keys:{relative_path}", text.slug),
                category: "architecture",
                severity: if findings.unconstrained_lookup_columns.len() >= 2 {
                    Severity::Medium
                } else {
                    Severity::Low
                },
                text: &text.missing_foreign_keys,
                evidence: format!(
                    "Detected lookup-style columns without local {} foreign key coverage: {}.",
                    text.name,
                    format_key_list(&findings.unconstrained_lookup_columns)
                ),
            },
        );
    }

    if !findings.non_unique_identity_columns.is_empty() {
        push_issue(
            issues,
            files,
            snapshot,
            IssueDraft {
                id: format!(
                    "local-{}-missing-unique-constraints:{relative_path}",
                    text.slug
                ),
                category: "architecture",
                severity: if findings.non_unique_identity_columns.len() >= 2 {
                    Severity::Medium
                } else {
                    Severity::Low
                },
                text: &text.missing_unique_constraints,
                evidence: format!(
                    "Detected identity-style columns without local {} unique coverage: {}.",
                    text.name,
                    format_key_list(&findings.non_unique_identity_columns)
                ),
            },
        );
    }

    if !findings.missing_composite_unique_tables.is_empty() {
        push_issue(
            issues,
            files,
            snapshot,
            IssueDraft {
                id: format!(
                    "local-{}-missing-composite-unique:{relative_path}",
                    text.slug
                ),
                category: "architecture",
                severity: Severity::Medium,
                text: &text.missing_composite_unique,
                evidence: format!(
                    "Detected join-style local {} tables without composite unique coverage: {}.",
                    text.name,
                    format_key_list(&findings.missing_composite_unique_tables)
                ),
            },
        );
    }

    if !findings.nullable_join_relation_columns.is_empty() {
        push_issue(
            issues,
            files,
            snapshot,
            IssueDraft {
                id: format!("local-{}-nullable-relations:{relative_path}", text.slug),
                category: "architecture",
                severity: Severity::Medium,
                text: &text.nullable_relations,
                evidence: format!(
                    "Detected join-style local {} tables with nullable relation columns: {}.",
                    text.name,
                    format_key_list(&findings.nullable_join_relation_columns)
                ),
            },
        );
    }
}

#[cfg(test)]
mod tests;
