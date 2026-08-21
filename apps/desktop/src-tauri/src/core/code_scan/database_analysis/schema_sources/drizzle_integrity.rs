use super::*;

pub(super) fn collect_drizzle_schema_integrity_issues(artifact: &TextArtifact) -> Vec<CodeIssue> {
    let mut issues = Vec::new();

    for capture in DRIZZLE_TABLE_DECL_PATTERN.captures_iter(&artifact.content) {
        let Some(table_var_name) = capture.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(table_name) = capture.get(2).map(|value| value.as_str()) else {
            continue;
        };
        let Some(body_match) = capture.get(3) else {
            continue;
        };
        let body = body_match.as_str();
        let table_line = line_number(
            &artifact.content,
            capture.get(0).map(|value| value.start()).unwrap_or(0),
        );
        let source_excerpt = excerpt_for_line(&artifact.content, Some(table_line));
        let mut lookup_fields = Vec::new();
        let mut referenced_lookup_fields = Vec::new();
        let mut nullable_lookup_fields = Vec::new();
        let mut disqualifying_fields = Vec::new();
        let mut missing_delete_intent_fields = Vec::new();

        for raw_line in body.lines() {
            let trimmed = raw_line
                .split("//")
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(',');
            if trimmed.is_empty() {
                continue;
            }

            let Some((field_name, rest)) = trimmed.split_once(':') else {
                continue;
            };
            let field_name = field_name.trim();
            let rest = rest.trim();
            if field_name.is_empty()
                || !field_name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                continue;
            }

            if rest.contains(".references(")
                && !referenced_lookup_fields
                    .iter()
                    .any(|existing| existing == field_name)
            {
                referenced_lookup_fields.push(field_name.to_string());
            }

            if DB_LOOKUP_FIELD_PATTERNS
                .iter()
                .any(|pattern| pattern.is_match(field_name))
            {
                lookup_fields.push(field_name.to_string());
                if !rest.contains(".notNull()") && !rest.contains(".primaryKey()") {
                    nullable_lookup_fields.push(field_name.to_string());
                }
                if rest.contains(".references(") && !rest.contains("onDelete:") {
                    missing_delete_intent_fields.push(field_name.to_string());
                }
                continue;
            }

            if field_name.eq_ignore_ascii_case("id") || is_join_like_metadata_column(field_name) {
                continue;
            }

            disqualifying_fields.push(field_name.to_string());
        }

        let is_join_like = lookup_fields.len() >= 2 && disqualifying_fields.is_empty();

        if !referenced_lookup_fields.is_empty() {
            let missing_index_fields = referenced_lookup_fields
                .iter()
                .filter(|field| {
                    !drizzle_schema_has_field_index(&artifact.content, table_var_name, field)
                })
                .cloned()
                .collect::<Vec<_>>();
            if !missing_index_fields.is_empty() && !is_join_like {
                issues.push(CodeIssue {
                    check_id: String::new(),
                    id: format!(
                        "schema-relation-missing-index:{}:{}",
                        artifact.relative_path,
                        sanitize_identifier(table_name)
                    ),
                    category: "data".into(),
                    severity: Severity::Low,
                    title: "Schema relation fields have no clear index coverage".into(),
                    description: "A Drizzle table in the scanned source defines relation references without an obvious `index(...)`, `uniqueIndex(...)`, or `primaryKey(...)` covering those fields. This may leave frequently joined or filtered queries without a supporting index, but query shape, generated migrations, database-created indexes, and constraints outside this file were not inspected.".into(),
                    relative_path: artifact.relative_path.clone(),
                    absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                    line: Some(table_line),
                    source_excerpt: source_excerpt.clone(),
                    evidence: Some(redact_evidence(format!(
                        "Drizzle table `{}` has relation fields {} without obvious index coverage in the scanned schema.",
                        table_name,
                        format_key_list(&missing_index_fields)
                    ))),
                    why_now: Some("If production queries filter or join on these fields at scale, missing index coverage can raise latency and database load; adding unused indexes also costs storage and write work.".into()),
                    likely_fix: Some("Inspect representative query plans and the applied database schema first. Add `index(...).on(...)` only for fields/combinations the workload uses, or document existing coverage supplied by an intentional unique/primary constraint, migration, or database-managed index.".into()),
                    confidence: crate::checks::IssueConfidence::NeedsReview,
                    confidence_reason: Some("Static Drizzle source shows no recognized index declaration, but query usage and the applied database/migration index set were not inspected.".into()),
                    verify_hint: Some("Apply the migration to a representative database and compare query plans plus read/write latency for the affected workload. Re-run Code Scan to confirm the intended declaration is visible, or mark the finding reviewed when equivalent coverage exists elsewhere.".into()),
                });
            }
        }

        if !is_join_like {
            continue;
        }

        if !drizzle_schema_has_composite_unique(&artifact.content, table_var_name, &lookup_fields) {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!(
                    "schema-join-missing-composite-unique:{}:{}",
                    artifact.relative_path,
                    sanitize_identifier(table_name)
                ),
                category: "architecture".into(),
                severity: Severity::Medium,
                title: "Schema join-style table is missing composite uniqueness".into(),
                description: "A Drizzle table in the scanned source looks like a membership or pivot table and has no recognized composite unique or primary-key constraint across its relation fields. That may allow duplicate logical memberships when the domain expects one row per relation pair, but repeated rows can be intentional and the scanner cannot infer the business key.".into(),
                relative_path: artifact.relative_path.clone(),
                absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                line: Some(table_line),
                source_excerpt: source_excerpt.clone(),
                evidence: Some(redact_evidence(format!(
                    "Drizzle table `{}` has relation-style fields {} without an obvious composite unique or primary-key definition covering them.",
                    table_name,
                    format_key_list(&lookup_fields)
                ))),
                why_now: Some("When the domain requires pair uniqueness, enforcing it in the database prevents concurrent writers and alternate code paths from creating duplicates; an incorrect constraint can instead reject legitimate history or multi-role rows.".into()),
                likely_fix: Some("Define the logical key with the product/data owner first. If one row per relation tuple is required, add a composite `uniqueIndex(...).on(...)` or `primaryKey({ columns: [...] })`, clean existing duplicates safely, and deploy the migration; otherwise document the repeated-row semantics.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The table shape is a join-table heuristic; static schema cannot determine whether relation tuples are supposed to be unique.".into()),
                verify_hint: Some("Test allowed and duplicate insert races against a migrated representative database, confirm existing data is handled, and verify the applied constraint matches the documented logical key.".into()),
            });
        }

        if !nullable_lookup_fields.is_empty() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!(
                    "schema-join-nullable-relations:{}:{}",
                    artifact.relative_path,
                    sanitize_identifier(table_name)
                ),
                category: "architecture".into(),
                severity: Severity::Medium,
                title: "Schema join-style table allows nullable relation fields".into(),
                description: "A Drizzle table in the scanned source looks like a membership or pivot table and has nullable relation-style fields. This permits partial relationships in the schema, but nullable links can be intentional for drafts, staged workflows, or soft association; requiredness must be checked against the domain and existing data.".into(),
                relative_path: artifact.relative_path.clone(),
                absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                line: Some(table_line),
                source_excerpt: source_excerpt.clone(),
                evidence: Some(redact_evidence(format!(
                    "Drizzle table `{}` has nullable relation-style fields {}.",
                    table_name,
                    format_key_list(&nullable_lookup_fields)
                ))),
                why_now: Some("If both sides are required by the domain, a database NOT NULL constraint protects every writer; changing optionality without reviewing existing rows can break valid workflows or fail migration.".into()),
                likely_fix: Some("Confirm the lifecycle and existing null rows first. Add `.notNull()` only when every valid row must have the relation, backfill/resolve existing data, and deploy the migration; otherwise document the intentional nullable state.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The join-table and requiredness assumptions are inferred from field shape; the scanner cannot inspect domain rules or all data states.".into()),
                verify_hint: Some("Test creation and transition flows against a migrated representative database, including existing null rows and concurrent writers, then confirm the applied nullability matches the documented lifecycle.".into()),
            });
        }

        if !missing_delete_intent_fields.is_empty() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!(
                    "schema-join-missing-delete-intent:{}:{}",
                    artifact.relative_path,
                    sanitize_identifier(table_name)
                ),
                category: "architecture".into(),
                severity: Severity::Low,
                title: "Schema join-style table has no explicit delete behavior".into(),
                description: "A Drizzle join-style table in the scanned source defines relation references without explicit delete behavior. The effective action depends on the installed Drizzle version/dialect, generated migration, and database foreign-key enforcement, so static source cannot determine whether current behavior matches product intent.".into(),
                relative_path: artifact.relative_path.clone(),
                absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                line: Some(table_line),
                source_excerpt,
                evidence: Some(redact_evidence(format!(
                    "Drizzle table `{}` has relation fields {} without an explicit `onDelete` policy in `.references(...)`.",
                    table_name,
                    format_key_list(&missing_delete_intent_fields)
                ))),
                why_now: Some("Unreviewed delete behavior can either block legitimate deletion or remove/retain related rows unexpectedly; making it explicit is useful only after ownership, retention, audit, and restore requirements are known.".into()),
                likely_fix: Some("Inspect the generated migration and applied foreign keys, then choose explicit delete behavior with the data owner. Account for retention, soft deletes, audit history, and existing rows before adding `onDelete`; do not default to cascade merely to clear the finding.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Missing explicit Drizzle source syntax is directly observed, but effective database behavior and intended lifecycle were not inspected.".into()),
                verify_hint: Some("Exercise parent deletion in a representative migrated database and confirm related rows, errors, audit records, and restore behavior match the documented policy. Re-run Code Scan after the intent is explicit or mark it reviewed when database-managed behavior is authoritative.".into()),
            });
        }
    }

    issues
}

fn drizzle_schema_has_composite_unique(
    content: &str,
    table_var_name: &str,
    lookup_fields: &[String],
) -> bool {
    let lower = content.to_ascii_lowercase();
    let table_var_name = table_var_name.to_ascii_lowercase();
    let relation_tokens = lookup_fields
        .iter()
        .map(|field| {
            let lower = field.to_ascii_lowercase();
            vec![
                format!("{}.{}", table_var_name, lower),
                format!(".{}", lower),
            ]
        })
        .collect::<Vec<_>>();

    for marker in ["uniqueindex", "primarykey"] {
        let mut search_start = 0usize;
        while let Some(relative_index) = lower[search_start..].find(marker) {
            let start = search_start + relative_index;
            let end = (start + 400).min(lower.len());
            let window = &lower[start..end];
            if relation_tokens.iter().all(|candidates| {
                candidates
                    .iter()
                    .any(|candidate| window.contains(candidate))
            }) {
                return true;
            }
            search_start = start + marker.len();
        }
    }

    false
}

fn drizzle_schema_has_field_index(content: &str, table_var_name: &str, field_name: &str) -> bool {
    let compact = content
        .to_ascii_lowercase()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let field_name = field_name.to_ascii_lowercase();
    let field_refs = [
        format!("{}.{}", table_var_name.to_ascii_lowercase(), field_name),
        format!("table.{}", field_name),
        format!(".{}", field_name),
    ];

    ((compact.contains("index(") || compact.contains("uniqueindex("))
        && field_refs.iter().any(|field_ref| {
            compact.contains(&format!(".on({field_ref})"))
                || compact.contains(&format!(".on({field_ref},"))
                || compact.contains(&format!(",{field_ref})"))
                || compact.contains(&format!(",{field_ref},"))
        }))
        || (compact.contains("primarykey(")
            && field_refs.iter().any(|field_ref| {
                compact.contains(&format!("columns:[{field_ref}]"))
                    || compact.contains(&format!("columns:[{field_ref},"))
                    || compact.contains(&format!(",{field_ref}]"))
                    || compact.contains(&format!(",{field_ref},"))
            }))
}
