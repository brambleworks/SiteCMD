use super::*;

pub(super) fn collect_prisma_schema_integrity_issues(artifact: &TextArtifact) -> Vec<CodeIssue> {
    let mut issues = Vec::new();
    let relative_lower = artifact.relative_path.to_ascii_lowercase();
    if !relative_lower.ends_with("schema.prisma") {
        return issues;
    }

    for capture in PRISMA_MODEL_BLOCK_PATTERN.captures_iter(&artifact.content) {
        let Some(model_name) = capture.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(body_match) = capture.get(2) else {
            continue;
        };
        let body = body_match.as_str();
        let model_line = line_number(
            &artifact.content,
            capture.get(0).map(|value| value.start()).unwrap_or(0),
        );
        let source_excerpt = excerpt_for_line(&artifact.content, Some(model_line));
        let mut lookup_fields = Vec::new();
        let mut relation_scalar_fields = Vec::new();
        let mut nullable_lookup_fields = Vec::new();
        let mut disqualifying_scalar_fields = Vec::new();
        let mut missing_delete_intent_relations = Vec::new();

        for raw_line in body.lines() {
            let trimmed = raw_line
                .split("//")
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(',');
            if trimmed.is_empty() || trimmed.starts_with("@@") {
                continue;
            }

            let mut parts = trimmed.split_whitespace();
            let Some(field_name) = parts.next() else {
                continue;
            };
            let Some(field_type) = parts.next() else {
                continue;
            };

            if trimmed.contains("@relation(") {
                if let Some(fields_segment) = trimmed
                    .split("fields: [")
                    .nth(1)
                    .and_then(|segment| segment.split(']').next())
                {
                    for field in fields_segment.split(',') {
                        let field = field.trim();
                        if !field.is_empty()
                            && !relation_scalar_fields
                                .iter()
                                .any(|existing| existing == field)
                        {
                            relation_scalar_fields.push(field.to_string());
                        }
                    }
                }
                if !field_type.contains('[') && !trimmed.contains("onDelete:") {
                    missing_delete_intent_relations.push(field_name.to_string());
                }
                continue;
            }

            if field_type.contains('[') {
                continue;
            }

            if DB_LOOKUP_FIELD_PATTERNS
                .iter()
                .any(|pattern| pattern.is_match(field_name))
            {
                lookup_fields.push(field_name.to_string());
                if field_type.ends_with('?') {
                    nullable_lookup_fields.push(field_name.to_string());
                }
                continue;
            }

            if field_name.eq_ignore_ascii_case("id")
                || is_join_like_metadata_column(field_name)
                || field_type
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_uppercase())
            {
                continue;
            }

            disqualifying_scalar_fields.push(field_name.to_string());
        }

        let is_join_like = lookup_fields.len() >= 2 && disqualifying_scalar_fields.is_empty();

        if !relation_scalar_fields.is_empty() {
            let missing_index_fields = relation_scalar_fields
                .iter()
                .filter(|field| !prisma_schema_has_field_index(body, field))
                .cloned()
                .collect::<Vec<_>>();
            if !missing_index_fields.is_empty() && !is_join_like {
                issues.push(CodeIssue {
                    check_id: String::new(),
                    id: format!(
                        "schema-relation-missing-index:{}:{}",
                        artifact.relative_path,
                        sanitize_identifier(model_name)
                    ),
                    category: "data".into(),
                    severity: Severity::Low,
                    title: "Schema relation fields have no clear index coverage".into(),
                    description: "A Prisma model in the scanned source defines relation scalar fields without an obvious `@@index`, `@@unique`, or `@@id` covering them. This may leave frequently joined or filtered queries without a supporting index, but query shape, database-created indexes, generated migrations, and constraints outside this file were not inspected.".into(),
                    relative_path: artifact.relative_path.clone(),
                    absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                    line: Some(model_line),
                    source_excerpt: source_excerpt.clone(),
                    evidence: Some(redact_evidence(format!(
                        "Prisma model `{}` has relation scalar fields {} without obvious index coverage in the scanned schema.",
                        model_name,
                        format_key_list(&missing_index_fields)
                    ))),
                    why_now: Some("If production queries filter or join on these fields at scale, missing index coverage can raise latency and database load; adding unused indexes also costs storage and write work.".into()),
                    likely_fix: Some("Inspect representative query plans and the applied database schema first. Add `@@index([..])` only for fields/combinations the workload uses, or document existing coverage supplied by an intentional `@@unique`, `@@id`, migration, or database-managed index.".into()),
                    confidence: crate::checks::IssueConfidence::NeedsReview,
                    confidence_reason: Some("Static Prisma source shows no recognized index declaration, but query usage and the applied database/migration index set were not inspected.".into()),
                    verify_hint: Some("Apply the migration to a representative database and compare query plans plus read/write latency for the affected workload. Re-run Code Scan to confirm the intended schema declaration is visible, or mark the finding reviewed when equivalent coverage exists elsewhere.".into()),
                });
            }
        }

        if !is_join_like {
            continue;
        }

        if !prisma_schema_has_composite_unique(body, &lookup_fields) {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!(
                    "schema-join-missing-composite-unique:{}:{}",
                    artifact.relative_path,
                    sanitize_identifier(model_name)
                ),
                category: "architecture".into(),
                severity: Severity::Medium,
                title: "Schema join-style model is missing composite uniqueness".into(),
                description: "A Prisma model in the scanned source looks like a membership or pivot table and has no recognized composite `@@unique` or `@@id` across its relation fields. That may allow duplicate logical memberships when the domain expects one row per relation pair, but repeated rows can be intentional and the scanner cannot infer the business key.".into(),
                relative_path: artifact.relative_path.clone(),
                absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                line: Some(model_line),
                source_excerpt: source_excerpt.clone(),
                evidence: Some(redact_evidence(format!(
                    "Prisma model `{}` has relation-style fields {} without an obvious composite `@@unique` or `@@id` covering them.",
                    model_name,
                    format_key_list(&lookup_fields)
                ))),
                why_now: Some("When the domain requires pair uniqueness, enforcing it in the database prevents concurrent writers and alternate code paths from creating duplicates; an incorrect constraint can instead reject legitimate history or multi-role rows.".into()),
                likely_fix: Some("Define the logical key with the product/data owner first. If one row per relation tuple is required, add a composite `@@unique([..])` or `@@id([..])`, clean existing duplicates safely, and deploy the generated migration; otherwise document the repeated-row semantics and mark the finding reviewed.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The model shape is a join-table heuristic; static schema cannot determine whether relation tuples are supposed to be unique.".into()),
                verify_hint: Some("Test allowed and duplicate insert races against a migrated representative database, confirm existing data is handled, and verify the applied constraint matches the documented logical key.".into()),
            });
        }

        if !nullable_lookup_fields.is_empty() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!(
                    "schema-join-nullable-relations:{}:{}",
                    artifact.relative_path,
                    sanitize_identifier(model_name)
                ),
                category: "architecture".into(),
                severity: Severity::Medium,
                title: "Schema join-style model allows nullable relation fields".into(),
                description: "A Prisma model in the scanned source looks like a membership or pivot table and has optional relation scalar fields. This permits partial relationships in the schema, but optional links can be intentional for drafts, staged workflows, or soft association; requiredness must be checked against the domain and existing data.".into(),
                relative_path: artifact.relative_path.clone(),
                absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                line: Some(model_line),
                source_excerpt: excerpt_for_line(&artifact.content, Some(model_line)),
                evidence: Some(redact_evidence(format!(
                    "Prisma model `{}` has nullable relation-style fields {}.",
                    model_name,
                    format_key_list(&nullable_lookup_fields)
                ))),
                why_now: Some("If both sides are required by the domain, a database NOT NULL constraint protects every writer; changing optionality without reviewing existing rows can break valid workflows or fail migration.".into()),
                likely_fix: Some("Confirm the lifecycle and existing null rows first. Make the relation scalar fields required only when every valid row must have both sides, backfill/resolve existing data, and deploy the generated migration; otherwise document the intentional optional state.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The join-table and requiredness assumptions are inferred from field shape; the scanner cannot inspect domain rules or all data states.".into()),
                verify_hint: Some("Test creation and transition flows against a migrated representative database, including existing null rows and concurrent writers, then confirm the applied nullability matches the documented lifecycle.".into()),
            });
        }

        if !missing_delete_intent_relations.is_empty() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!(
                    "schema-join-missing-delete-intent:{}:{}",
                    artifact.relative_path,
                    sanitize_identifier(model_name)
                ),
                category: "architecture".into(),
                severity: Severity::Low,
                title: "Schema join-style model has no explicit delete behavior".into(),
                description: "A Prisma join-style model in the scanned source defines relations without explicit `onDelete` behavior. The effective action depends on the installed Prisma version, provider, relation mode, generated migration, and database constraints, so static source cannot determine whether current delete behavior matches product intent.".into(),
                relative_path: artifact.relative_path.clone(),
                absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                line: Some(model_line),
                source_excerpt: excerpt_for_line(&artifact.content, Some(model_line)),
                evidence: Some(redact_evidence(format!(
                    "Prisma model `{}` has relation fields {} without an explicit `onDelete:` policy.",
                    model_name,
                    format_key_list(&missing_delete_intent_relations)
                ))),
                why_now: Some("Unreviewed delete behavior can either block legitimate deletion or remove/retain related rows unexpectedly; making it explicit is useful only after ownership, retention, audit, and restore requirements are known.".into()),
                likely_fix: Some("Inspect the generated migration and applied foreign keys, then choose explicit delete behavior with the data owner. Account for retention, soft deletes, audit history, and existing rows before adding `onDelete`; do not default to cascade merely to clear the finding.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Missing explicit Prisma source syntax is directly observed, but effective database behavior and intended lifecycle were not inspected.".into()),
                verify_hint: Some("Exercise parent deletion in a representative migrated database and confirm related rows, errors, audit records, and restore behavior match the documented policy. Re-run Code Scan after the intent is explicit or mark it reviewed when database-managed behavior is authoritative.".into()),
            });
        }
    }

    issues
}

fn prisma_schema_has_composite_unique(body: &str, lookup_fields: &[String]) -> bool {
    let lookup_fields = lookup_fields
        .iter()
        .map(|field| field.to_ascii_lowercase())
        .collect::<Vec<_>>();

    body.lines().any(|line| {
        let trimmed = line.trim().to_ascii_lowercase();
        (trimmed.contains("@@unique([") || trimmed.contains("@@id(["))
            && lookup_fields
                .iter()
                .all(|field| trimmed.contains(field.as_str()))
    })
}

fn prisma_schema_has_field_index(body: &str, field: &str) -> bool {
    let field_lower = field.to_ascii_lowercase();

    body.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with(&format!("{field_lower} ")) {
            return lower.contains("@id") || lower.contains("@unique");
        }
        (lower.starts_with("@@index([")
            || lower.starts_with("@@unique([")
            || lower.starts_with("@@id(["))
            && extract_bracket_fields(&lower)
                .iter()
                .any(|candidate| candidate == &field_lower)
    })
}

fn extract_bracket_fields(value: &str) -> Vec<String> {
    value
        .split('[')
        .nth(1)
        .and_then(|segment| segment.split(']').next())
        .map(|segment| {
            segment
                .split(',')
                .map(|field| field.trim().to_ascii_lowercase())
                .filter(|field| !field.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
