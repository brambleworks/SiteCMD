use super::*;

pub(super) fn collect_supabase_policy_issues(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    frontend_supabase_accesses: &[SupabaseTableAccess],
    frontend_supabase_files: &[&SourceFile],
    local_rls_states: &HashMap<String, LocalRlsTableState>,
    has_local_rls_markers: bool,
    db_lookup_fields: &[String],
    has_db_index_hints: bool,
    database_artifacts: &[TextArtifact],
) {
    // Include the table in issue ids and deduplicate each policy/table pair.
    let mut seen_policy_ids: HashSet<String> = HashSet::new();
    if !frontend_supabase_accesses.is_empty() {
        for access in frontend_supabase_accesses {
            let Some(file) = files
                .iter()
                .find(|file| file.relative_path == access.relative_path)
            else {
                continue;
            };
            let table_state = local_rls_states.get(&access.table);

            if table_state.is_none() || !table_state.is_some_and(|state| state.enabled) {
                let mut issue = build_issue(
                    "supabase-rls-missing",
                    "security",
                    Severity::Medium,
                    "No recognized local RLS enablement for a client-accessed Supabase table",
                    "This frontend surface appears to query a Supabase/Postgres table, but the scanned local artifacts do not show recognized Row Level Security enablement for that same table. The deployed database was not inspected, and custom, generated, remote-only, or parser-unsupported policy management may provide coverage. If the deployed client-accessible table really has RLS disabled, database grants rather than per-row policies govern access.",
                    file,
                    access.line,
                    Some(format!(
                        "Frontend code appears to access Supabase table `{}`, but scanned local database artifacts do not show a recognized `ALTER TABLE ... ENABLE ROW LEVEL SECURITY` statement for that table; deployed state was not inspected.",
                        access.table
                    )),
                    Some("Inspect the applied schema in an isolated local or staging Supabase project first. If the client-accessed table lacks RLS, add an authoritative migration that enables it and define only the required role- and row-scoped policies. If policy management intentionally lives elsewhere, document that source and mark the local-artifact finding not applicable.".into()),
                    Some("As anon and authenticated test roles, exercise intended and forbidden rows in an isolated environment; inspect `pg_class.relrowsecurity`, applicable policies, roles, and grants, then re-run Code Scan against the authoritative local artifacts.".into()),
                );
                issue.id = format!(
                    "supabase-rls-missing:{}:{}",
                    access.relative_path, access.table
                );
                issues.push(issue);
                continue;
            }

            let state = table_state.expect("table_state is Some after the None branch above");
            if state.policies.is_empty() {
                let mut issue = build_issue(
                    "supabase-policy-set-empty",
                    "data",
                    Severity::Medium,
                    "Client-facing Supabase table has RLS enabled but no local policies",
                    "The local schema enables Row Level Security for this client-facing table but defines no table policy. PostgreSQL defaults to denying rows to ordinary client roles in that state, so the frontend operation is likely to return no rows or a permission error. This is not an RLS bypass; it is a configuration or incomplete-migration signal.",
                    file,
                    access.line,
                    Some(format!(
                        "Frontend queries Supabase table `{}`, while local artifacts enable RLS and contain no table-specific CREATE POLICY statement.",
                        access.table
                    )),
                    Some("Confirm the table is intended to be client-accessible. If it is, add only the SELECT or write policies the frontend actually needs; if it is intentionally server-only, remove the browser access instead of weakening RLS.".into()),
                    Some("Exercise the detected frontend operation with the anon or authenticated role in a local Supabase environment and confirm the intended allow or deny behavior.".into()),
                );
                issue.id = format!(
                    "supabase-policy-set-empty:{}:{}",
                    access.relative_path, access.table
                );
                issues.push(issue);
            }

            let required_operations = access
                .operations
                .iter()
                .flat_map(|operation| match operation.as_str() {
                    "upsert" => vec![String::from("insert"), String::from("update")],
                    other => vec![other.to_string()],
                })
                .collect::<HashSet<_>>();
            let covered_operations = state
                .policies
                .iter()
                .filter(|policy| policy.applies_to_frontend_roles)
                .flat_map(|policy| policy.operations.iter().cloned())
                .collect::<HashSet<_>>();
            let missing_operations = required_operations
                .iter()
                .filter(|operation| !covered_operations.contains(operation.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            if !state.policies.is_empty() && !missing_operations.is_empty() {
                let mut sorted_missing_operations = missing_operations;
                sorted_missing_operations.sort();
                let mut issue = build_issue(
                    "supabase-policy-operation-missing",
                    "data",
                    Severity::Medium,
                    "Client-facing Supabase operation has no matching local RLS policy",
                    "The frontend appears to perform a Supabase operation that the scanned local policy set does not grant to a frontend-capable role. With RLS enabled, that operation would normally be denied if applied policies and grants match the scanned artifacts; this is not an access-control bypass. The deployed database, runtime role, grants, generated policies, and remote-only migrations were not inspected.",
                    file,
                    access.line,
                    Some(format!(
                        "Frontend uses Supabase table `{}` with operations {}, but local policies only cover {}.",
                        access.table,
                        format_key_list(&required_operations.iter().cloned().collect::<Vec<_>>()),
                        if covered_operations.is_empty() {
                            String::from("nothing explicit")
                        } else {
                            format_key_list(&covered_operations.iter().cloned().collect::<Vec<_>>())
                        }
                    )),
                    Some(format!(
                        "Confirm the operation is intended, then add narrowly scoped local policy coverage only for {}. If the operation should be server-only, move it behind a trusted server boundary instead.",
                        format_key_list(&sorted_missing_operations)
                    )),
                    Some("Test each detected operation in an isolated environment as the same anon or authenticated role the frontend uses. Confirm the applied policy and grants intentionally allow or deny it, then re-run Code Scan after the authoritative local artifacts represent that behavior.".into()),
                );
                issue.id = format!(
                    "supabase-policy-operation-missing:{}:{}",
                    access.relative_path, access.table
                );
                issues.push(issue);
            }

            for policy in &state.policies {
                if !policy.applies_to_frontend_roles {
                    continue;
                }
                if policy.permissive {
                    let id = format!(
                        "supabase-open-policy:{}:{}:{}",
                        policy.relative_path,
                        access.table,
                        policy.line.unwrap_or(0)
                    );
                    // One finding per policy/table/line: without the line, two
                    // policies for the same table in one migration collapse;
                    // without this set, each accessing file re-emits them.
                    if !seen_policy_ids.insert(id.clone()) {
                        continue;
                    }
                    // "appears to allow" is a heuristic SQL read: graded by
                    // the shared confidence policy (NeedsReview).
                    let (confidence, confidence_reason) = policy_confidence("supabase-open-policy");
                    let allows_write = policy
                        .operations
                        .iter()
                        .any(|operation| operation != "select");
                    let (severity, title, description, why_now, likely_fix) = if allows_write {
                        (
                            Severity::High,
                            "Client-facing Supabase write policy has an unconditional row condition",
                            "A local INSERT, UPDATE, or DELETE policy for a frontend-accessed table contains `USING (true)` or `WITH CHECK (true)`. One condition is unconditional, but effective access can still depend on the other policy clause, the policy's TO roles, and table grants. Review those controls together before deciding whether the write is broader than intended.",
                            "An unconditional write condition can widen which existing rows are targetable or which new row values are accepted, depending on the operation and the policy's other clause.",
                            "Review USING and WITH CHECK separately for the detected operation, plus the TO roles and table grants. Replace only the unconditional condition that conflicts with the intended owner, tenant, state, or role boundary; document deliberately broad writes.",
                        )
                    } else {
                        (
                            Severity::Medium,
                            "Client-facing Supabase SELECT policy has an unconditional row condition",
                            "A local SELECT policy for a frontend-accessed table contains `USING (true)`, so this policy contributes no row restriction for roles it covers. Effective visibility also depends on the policy's roles, table grants, other permissive policies, any restrictive policies, and the applied database state. An unconditional policy can be correct for intentionally public data.",
                            "A deliberately public-read policy should match the table's data classification; later sensitive columns, changed grants, or policy composition can alter the effective exposure.",
                            "Review the policy's roles, table grants, other permissive and restrictive policies, exposed columns, and intended data classification. Keep and document it for genuinely public data; otherwise add the required visibility, owner, tenant, or role predicate.",
                        )
                    };
                    issues.push(CodeIssue {
                        check_id: String::new(),
                        id,
                        category: "security".into(),
                        severity,
                        title: title.into(),
                        description: description.into(),
                        relative_path: policy.relative_path.clone(),
                        absolute_path: policy.absolute_path.clone(),
                        line: policy.line,
                        source_excerpt: excerpt_for_line(
                            file_content_for_relative(files, &policy.relative_path),
                            policy.line,
                        ),
                        evidence: Some(redact_evidence(format!(
                            "Frontend queries table `{}`, and the policy at line {} for roles {} contains an unconditional condition for {}.",
                            access.table,
                            policy.line.unwrap_or(0),
                            format_key_list(&policy.roles),
                            format_key_list(&policy.operations.iter().cloned().collect::<Vec<_>>())
                        ))),
                        why_now: Some(why_now.into()),
                        likely_fix: Some(likely_fix.into()),
                        confidence,
                        confidence_reason,
                        verify_hint: Some(if allows_write {
                            "Test the write with the policy's exact role and with a different user or tenant. After tightening it, re-run Code Scan and confirm the unconditional-write finding clears."
                        } else {
                            "Test the SELECT as each covered role and review every exposed column. If public access is intentional, document that classification and mark the finding not applicable; otherwise tighten the policy and re-scan."
                        }.into()),
                    });
                } else if !policy.auth_scoped
                    && policy
                        .operations
                        .iter()
                        .any(|operation| operation != "select")
                {
                    let id = format!(
                        "supabase-policy-not-auth-scoped:{}:{}:{}",
                        policy.relative_path,
                        access.table,
                        policy.line.unwrap_or(0)
                    );
                    // Same dedupe as supabase-open-policy: distinguish policy
                    // lines but do not repeat them for each accessing file.
                    if !seen_policy_ids.insert(id.clone()) {
                        continue;
                    }
                    let (confidence, confidence_reason) =
                        policy_confidence("supabase-policy-not-auth-scoped");
                    issues.push(CodeIssue {
                        check_id: String::new(),
                        id,
                        category: "security".into(),
                        severity: Severity::Medium,
                        title: "Client-facing Supabase write policy has no clear per-row caller boundary".into(),
                        description: "A write policy in scanned local SQL has a row predicate, but the recognized clause text does not visibly bind the write to the current user, tenant, or JWT claim. Static analysis cannot prove the policy is unsafe: a restricted `TO` role, grants, a security-definer boundary, another restrictive policy, or an application invariant may supply the intended control.".into(),
                        relative_path: policy.relative_path.clone(),
                        absolute_path: policy.absolute_path.clone(),
                        line: policy.line,
                        source_excerpt: excerpt_for_line(
                            file_content_for_relative(files, &policy.relative_path),
                            policy.line,
                        ),
                        evidence: Some(redact_evidence(format!(
                            "Frontend queries table `{}`, but the matching local policy does not show obvious auth or claim-based scoping.",
                            access.table
                        ))),
                        why_now: Some("Write policies protect integrity across users and tenants; a business-state predicate alone may allow any covered caller to modify every matching row.".into()),
                        likely_fix: Some("Review the policy's TO roles, table grants, USING clause, and WITH CHECK clause together. If caller isolation is required, add the exact user, tenant, role, or claim boundary; do not add auth.uid() to intentionally public SELECT policies.".into()),
                        confidence,
                        confidence_reason,
                        verify_hint: Some("In an isolated environment, test the write with the policy's exact roles and with two users or tenants, inspect the applied grants and policy composition, and confirm one caller cannot alter another's rows unless that is explicitly intended.".into()),
                    });
                }
            }
        }
    } else if !frontend_supabase_files.is_empty() && !has_local_rls_markers {
        if let Some(file) = frontend_supabase_files.first() {
            issues.push(build_issue(
                "supabase-rls-missing",
                "security",
                Severity::Medium,
                "Client-facing Supabase usage has no recognized local RLS artifacts",
                "Client-facing Supabase usage was detected, but the scanned local artifacts contain no recognized RLS enablement or policy statements. The table names or operations could not be correlated in this fallback path, and the deployed database was not inspected; remote-only, generated, or parser-unsupported policy management may exist.",
                file,
                first_match_line(&file.content, &DB_PATTERNS),
                Some("Supabase usage was detected in a frontend surface, but no ENABLE ROW LEVEL SECURITY or CREATE POLICY markers were found in local database artifacts.".into()),
                Some("Identify the exact tables and operations used by the browser, then inspect their applied RLS state, policies, roles, and grants in an isolated local or staging project. If coverage is missing, add authoritative local migrations with least-privilege policies; otherwise document the external policy source.".into()),
                Some("Exercise each browser operation as anon and authenticated test roles in an isolated environment, confirm intended rows are allowed and cross-user or cross-tenant rows are denied, and ensure the authoritative local artifacts reproduce the applied policy set.".into()),
            ));
        }
    }

    for file in frontend_supabase_files {
        // Require executable createClient calls; code samples in template
        // literals are not browser client initialization.
        if has_create_client_outside_template_literal(&file.content)
            && has_any(&file.content, &SUPABASE_SERVICE_ROLE_PATTERNS)
        {
            issues.push(build_issue(
                "supabase-service-role-client",
                "security",
                Severity::High,
                "Client-facing Supabase code references a service-role credential name",
                "This client-facing file initializes Supabase near a service-role-named value. A live service-role credential in a browser bundle would bypass RLS, but this source match does not prove that a real value is configured, that the module reaches the client build, or that the deployed bundle contains it.",
                file,
                first_match_line(&file.content, &SUPABASE_SERVICE_ROLE_PATTERNS),
                Some("A createClient call in a client-facing file appears near a service-role literal or environment-variable name. No configured value or production bundle was inspected.".into()),
                Some("First inspect the resolved production client build and deployment configuration without exposing the value. If a real service-role credential reached any client asset, repository, or log, revoke or rotate it immediately. Use only the publishable/anon key in the browser with tested RLS policies, and move privileged operations behind an authenticated, authorized server boundary using the least privilege available.".into()),
                Some("Build with controlled production configuration, inspect emitted client assets for both the configured value and service-role reference, and verify the old key fails if exposure was confirmed. Exercise the browser flow as unauthenticated and as two users to confirm RLS and server authorization enforce the intended boundaries.".into()),
            ));
        }
    }

    if db_lookup_fields.len() >= 2 && !has_db_index_hints {
        if let Some(anchor) = database_artifacts.first() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("db-index-hints-missing:{}", anchor.relative_path),
                category: "architecture".into(),
                severity: Severity::Medium,
                title: "Lookup-shaped schema fields have no recognized index declarations".into(),
                description: "The scanned schema or migration artifacts define multiple fields whose names resemble lookup or foreign-key columns, but no recognized secondary index declaration was found. This is a review prompt, not proof of a missing index: the scan does not inspect the query workload, data distribution, database-generated indexes, applied schema, or query plans, and unnecessary indexes carry write and storage cost.".into(),
                relative_path: anchor.relative_path.clone(),
                absolute_path: anchor.absolute_path.to_string_lossy().to_string(),
                line: find_first_lookup_field_line(&anchor.content),
                source_excerpt: excerpt_for_line(&anchor.content, find_first_lookup_field_line(&anchor.content)),
                evidence: Some(redact_evidence(format!(
                    "Detected lookup-style schema fields {} but found no @@index, CREATE INDEX, uniqueIndex, or similar local index hints.",
                    format_key_list(db_lookup_fields)
                ))),
                why_now: Some("Frequently filtered or joined columns can become expensive as data grows, while speculative indexes slow writes and consume storage. The right decision depends on actual query shapes and selectivity.".into()),
                likely_fix: Some("Identify the queries that filter or join on the named fields, then use representative local data and `EXPLAIN` or the database's query-plan tooling. Add an index through the authoritative schema or migrations only where the plan and workload justify it; otherwise mark the field not applicable.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Field names and missing recognized declarations are static heuristics; query workload, applied indexes, selectivity, and planner behavior were not inspected.".into()),
                verify_hint: Some("Compare representative query plans and timings before and after any index change, confirm write performance remains acceptable, and verify the applied local schema contains only the indexes the workload needs.".into()),
            });
        }
    }
}

/// Detect `createClient` outside backtick code samples.
/// Nested template backticks are intentionally unsupported.
fn has_create_client_outside_template_literal(content: &str) -> bool {
    let bytes = content.as_bytes();
    let needle = b"createClient";
    let mut in_backtick = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'`' => in_backtick = !in_backtick,
            _ => {
                if !in_backtick && bytes[i..].starts_with(needle) {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod service_role_tests {
    use super::has_create_client_outside_template_literal;

    #[test]
    fn ignores_create_client_inside_a_code_sample_string() {
        // A marketing showcase that displays Supabase code as a string.
        let display = "const code = `import { createClient } from 'supabase-js'\n\
            const supabase = createClient(url, Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!)`;\n\
            export default function Panel() { return <Code>{code}</Code> }";
        assert!(!has_create_client_outside_template_literal(display));
    }

    #[test]
    fn detects_a_real_create_client_call() {
        let real = "import { createClient } from 'supabase-js'\n\
            const supabase = createClient(url, process.env.SUPABASE_SERVICE_ROLE_KEY)";
        assert!(has_create_client_outside_template_literal(real));
    }
}
