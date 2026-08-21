use super::*;

pub(in crate::core::code_scan) fn collect_db_lookup_fields(
    artifacts: &[TextArtifact],
) -> Vec<String> {
    let mut fields = HashSet::new();

    for artifact in artifacts {
        for pattern in DB_LOOKUP_FIELD_PATTERNS.iter() {
            for capture in pattern.captures_iter(&artifact.content) {
                let Some(field) = capture.get(1).map(|value| value.as_str().to_string()) else {
                    continue;
                };
                fields.insert(field);
            }
        }
    }

    let mut collected = fields.into_iter().collect::<Vec<_>>();
    collected.sort();
    collected
}

pub(in crate::core::code_scan) fn collect_frontend_supabase_table_accesses(
    files: &[&SourceFile],
) -> Vec<SupabaseTableAccess> {
    let mut accesses_by_key: HashMap<String, SupabaseTableAccess> = HashMap::new();

    for file in files {
        for capture in SUPABASE_TABLE_OPERATION_PATTERN.captures_iter(&file.content) {
            let Some(table_match) = capture.get(1) else {
                continue;
            };
            let Some(operation_match) = capture.get(2) else {
                continue;
            };
            let table = normalize_table_identifier(table_match.as_str());
            if table.is_empty() {
                continue;
            }
            let operation = operation_match.as_str().to_ascii_lowercase();
            let key = format!("{}:{}", file.relative_path, table);
            accesses_by_key
                .entry(key)
                .and_modify(|access| {
                    access.operations.insert(operation.clone());
                })
                .or_insert_with(|| SupabaseTableAccess {
                    relative_path: file.relative_path.clone(),
                    table,
                    line: Some(line_number(&file.content, table_match.start())),
                    operations: HashSet::from([operation]),
                });
        }

        for capture in SUPABASE_FROM_TABLE_PATTERN.captures_iter(&file.content) {
            let Some(table_match) = capture.get(1) else {
                continue;
            };
            let table = normalize_table_identifier(table_match.as_str());
            if table.is_empty() {
                continue;
            }
            let key = format!("{}:{}", file.relative_path, table);
            accesses_by_key
                .entry(key)
                .or_insert_with(|| SupabaseTableAccess {
                    relative_path: file.relative_path.clone(),
                    table,
                    line: Some(line_number(&file.content, table_match.start())),
                    operations: HashSet::from([String::from("select")]),
                });
        }
    }

    let mut accesses = accesses_by_key.into_values().collect::<Vec<_>>();
    accesses.sort_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.table.cmp(&right.table))
    });
    accesses
}

pub(in crate::core::code_scan) fn collect_local_rls_table_states(
    artifacts: &[TextArtifact],
) -> HashMap<String, LocalRlsTableState> {
    let mut states = HashMap::new();

    for artifact in artifacts {
        for capture in SQL_RLS_ENABLE_PATTERN.captures_iter(&artifact.content) {
            let Some(table_match) = capture.get(1) else {
                continue;
            };
            let table = normalize_table_identifier(table_match.as_str());
            if table.is_empty() {
                continue;
            }
            states
                .entry(table)
                .or_insert_with(LocalRlsTableState::default)
                .enabled = true;
        }

        for capture in SQL_CREATE_POLICY_PATTERN.captures_iter(&artifact.content) {
            let Some(table_match) = capture.get(1) else {
                continue;
            };
            let Some(policy_match) = capture.get(0) else {
                continue;
            };
            let table = normalize_table_identifier(table_match.as_str());
            if table.is_empty() {
                continue;
            }
            let policy_clauses = capture.get(2).map(|value| value.as_str()).unwrap_or("");
            let roles = collect_policy_roles(policy_clauses);
            let entry = states
                .entry(table)
                .or_insert_with(LocalRlsTableState::default);
            entry.policies.push(LocalRlsPolicyState {
                relative_path: artifact.relative_path.clone(),
                absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                line: Some(line_number(&artifact.content, policy_match.start())),
                auth_scoped: has_any(policy_clauses, &RLS_AUTH_SCOPING_PATTERNS),
                permissive: has_any(policy_clauses, &PERMISSIVE_RLS_POLICY_PATTERNS),
                operations: collect_policy_operations(policy_clauses),
                applies_to_frontend_roles: policy_roles_may_reach_frontend(&roles),
                roles,
            });
        }
    }

    states
}

fn collect_policy_roles(policy_clauses: &str) -> Vec<String> {
    let Some(capture) = SQL_POLICY_ROLE_CLAUSE_PATTERN.captures(policy_clauses) else {
        return vec![String::from("public")];
    };
    let mut roles = capture[1]
        .split(',')
        .map(|role| {
            role.trim()
                .trim_matches(['\'', '"', '`'])
                .to_ascii_lowercase()
        })
        .filter(|role| !role.is_empty())
        .collect::<Vec<_>>();
    if roles.is_empty() {
        roles.push(String::from("public"));
    }
    roles
}

fn policy_roles_may_reach_frontend(roles: &[String]) -> bool {
    // Supabase's service_role is explicitly server-only and bypasses RLS.
    // Unknown/custom roles remain reviewable because a JWT can deliberately
    // map a browser session to a custom database role.
    !roles.iter().all(|role| role == "service_role")
}

fn normalize_table_identifier(value: &str) -> String {
    value
        .trim()
        .trim_matches(['"', '`', '\''])
        .split('.')
        .next_back()
        .unwrap_or("")
        .trim_matches(['"', '`', '\''])
        .to_ascii_lowercase()
}

fn collect_policy_operations(policy_sql: &str) -> HashSet<String> {
    let mut operations = SQL_POLICY_OPERATION_PATTERN
        .captures_iter(policy_sql)
        .filter_map(|capture| {
            capture
                .get(1)
                .map(|value| value.as_str().to_ascii_lowercase())
        })
        .collect::<HashSet<_>>();

    if operations.is_empty() || operations.contains("all") {
        return HashSet::from([
            String::from("select"),
            String::from("insert"),
            String::from("update"),
            String::from("delete"),
        ]);
    }

    if operations.contains("upsert") {
        operations.insert(String::from("insert"));
        operations.insert(String::from("update"));
        operations.remove("upsert");
    }

    operations
}

pub(in crate::core::code_scan) fn find_first_lookup_field_line(content: &str) -> Option<u32> {
    DB_LOOKUP_FIELD_PATTERNS.iter().find_map(|pattern| {
        pattern
            .find(content)
            .map(|matched| line_number(content, matched.start()))
    })
}
