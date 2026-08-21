use super::*;

/// Capture a Drizzle builder's explicit SQL column name.
/// The TypeScript property key is only the fallback for no-argument builders.
static DRIZZLE_COLUMN_NAME_ARG: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(
    || {
        regex::Regex::new(
            r#"^\s*[A-Za-z_$][A-Za-z0-9_$]*(?:\s*\.\s*[A-Za-z_$][A-Za-z0-9_$]*)*\s*\(\s*["']([A-Za-z0-9_]+)["']"#,
        )
        .expect("static drizzle column-name regex") // allow-expect: compile-time literal regex
    },
);

pub(in crate::core::code_scan) fn collect_expected_db_table_names(
    artifacts: &[TextArtifact],
) -> Vec<String> {
    let mut tables = HashSet::new();

    for artifact in artifacts {
        let relative_lower = artifact.relative_path.to_ascii_lowercase();

        if relative_lower.ends_with("schema.prisma") {
            for capture in PRISMA_MODEL_BLOCK_PATTERN.captures_iter(&artifact.content) {
                let Some(model_name) = capture.get(1).map(|value| value.as_str()) else {
                    continue;
                };
                let body = capture.get(2).map(|value| value.as_str()).unwrap_or("");
                let mapped = PRISMA_TABLE_MAP_PATTERN
                    .captures(body)
                    .and_then(|mapped_capture| mapped_capture.get(1).map(|value| value.as_str()));
                tables.insert(mapped.unwrap_or(model_name).to_ascii_lowercase());
            }
        }

        for capture in SQL_CREATE_TABLE_PATTERN.captures_iter(&artifact.content) {
            if let Some(table_name) = capture
                .get(1)
                .map(|value| value.as_str().to_ascii_lowercase())
            {
                tables.insert(table_name);
            }
        }

        for capture in DRIZZLE_TABLE_PATTERN.captures_iter(&artifact.content) {
            if let Some(table_name) = capture
                .get(1)
                .map(|value| value.as_str().to_ascii_lowercase())
            {
                tables.insert(table_name);
            }
        }
    }

    let mut collected = tables.into_iter().collect::<Vec<_>>();
    collected.sort();
    collected
}

pub(in crate::core::code_scan) fn collect_expected_db_columns(
    artifacts: &[TextArtifact],
) -> HashMap<String, HashSet<String>> {
    let mut tables = HashMap::new();

    for artifact in artifacts {
        let relative_lower = artifact.relative_path.to_ascii_lowercase();

        if relative_lower.ends_with("schema.prisma") {
            for capture in PRISMA_MODEL_BLOCK_PATTERN.captures_iter(&artifact.content) {
                let Some(model_name) = capture.get(1).map(|value| value.as_str()) else {
                    continue;
                };
                let body = capture.get(2).map(|value| value.as_str()).unwrap_or("");
                let mapped = PRISMA_TABLE_MAP_PATTERN
                    .captures(body)
                    .and_then(|mapped_capture| mapped_capture.get(1).map(|value| value.as_str()));
                let table_name = mapped.unwrap_or(model_name).to_ascii_lowercase();
                let entry = tables.entry(table_name).or_insert_with(HashSet::new);

                for line in body.lines() {
                    let trimmed = line
                        .split("//")
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_end_matches(',');
                    if trimmed.is_empty() || trimmed.starts_with("@@") || trimmed.starts_with("//")
                    {
                        continue;
                    }

                    let mut parts = trimmed.split_whitespace();
                    let Some(field_name) = parts.next() else {
                        continue;
                    };
                    let Some(field_type) = parts.next() else {
                        continue;
                    };
                    if !field_name
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
                    {
                        continue;
                    }
                    // Relation fields are not database columns: the object side
                    // carries @relation, and the list side is a `Type[]`. The FK
                    // scalar (authorId Int) is a separate line and still counts.
                    // Mirrors the filtering in prisma_integrity.rs.
                    if trimmed.contains("@relation(") || field_type.contains('[') {
                        continue;
                    }

                    let mapped_field =
                        PRISMA_FIELD_MAP_PATTERN
                            .captures(trimmed)
                            .and_then(|mapped_capture| {
                                mapped_capture.get(1).map(|value| value.as_str())
                            });
                    entry.insert(mapped_field.unwrap_or(field_name).to_ascii_lowercase());
                }
            }
        }

        for capture in SQL_CREATE_TABLE_BLOCK_PATTERN.captures_iter(&artifact.content) {
            let Some(table_name) = capture
                .get(1)
                .map(|value| value.as_str().to_ascii_lowercase())
            else {
                continue;
            };
            let body = capture.get(2).map(|value| value.as_str()).unwrap_or("");
            let entry = tables.entry(table_name).or_insert_with(HashSet::new);

            for raw_line in body.lines() {
                let trimmed = raw_line
                    .split("--")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches(',');
                if trimmed.is_empty() {
                    continue;
                }
                let lower = trimmed.to_ascii_lowercase();
                if lower.starts_with("constraint ")
                    || lower.starts_with("primary ")
                    || lower.starts_with("foreign ")
                    || lower.starts_with("unique ")
                    || lower.starts_with("check ")
                    || lower.starts_with("key ")
                {
                    continue;
                }

                let Some(column_token) = trimmed.split_whitespace().next() else {
                    continue;
                };
                let column_name = column_token.trim_matches(['"', '`', '\'']);
                if column_name.is_empty() {
                    continue;
                }
                entry.insert(column_name.to_ascii_lowercase());
            }
        }

        for capture in DRIZZLE_TABLE_BLOCK_PATTERN.captures_iter(&artifact.content) {
            let Some(table_name) = capture
                .get(1)
                .map(|value| value.as_str().to_ascii_lowercase())
            else {
                continue;
            };
            let body = capture.get(2).map(|value| value.as_str()).unwrap_or("");
            let entry = tables.entry(table_name).or_insert_with(HashSet::new);

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
                let Some((field_name, builder)) = trimmed.split_once(':') else {
                    continue;
                };
                let field_name = field_name.trim();
                if field_name.is_empty()
                    || !field_name
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
                {
                    continue;
                }
                // The SQL column name is the builder call's first string
                // argument when present (see DRIZZLE_COLUMN_NAME_ARG); the TS
                // property key is only the fallback for the no-argument form.
                let column_name = DRIZZLE_COLUMN_NAME_ARG
                    .captures(builder)
                    .and_then(|capture| capture.get(1))
                    .map(|column| column.as_str())
                    .unwrap_or(field_name);
                entry.insert(column_name.to_ascii_lowercase());
            }
        }
    }

    tables
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(relative_path: &str, content: &str) -> TextArtifact {
        TextArtifact {
            absolute_path: std::path::PathBuf::from(relative_path),
            relative_path: relative_path.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn schema_qualified_create_table_captures_table_not_schema() {
        let artifacts = vec![artifact(
            "supabase/migrations/0001_init.sql",
            "create table public.users (\n  id serial primary key,\n  name text\n);",
        )];

        let names = collect_expected_db_table_names(&artifacts);
        assert!(
            names.contains(&"users".to_string()),
            "expected table `users`, got {names:?}"
        );
        assert!(
            !names.contains(&"public".to_string()),
            "the schema qualifier must not be captured as a table, got {names:?}"
        );

        let columns = collect_expected_db_columns(&artifacts);
        let user_columns = columns.get("users").expect("users columns");
        assert!(user_columns.contains("id") && user_columns.contains("name"));
        assert!(
            !columns.contains_key("public"),
            "no phantom `public` table, got {:?}",
            columns.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn drizzle_expected_columns_use_the_builder_column_name_not_the_ts_key() {
        let artifacts = vec![artifact(
            "src/db/schema.ts",
            r#"
                export const users = sqliteTable('users', {
                  id: integer('id').primaryKey(),
                  fullName: text('full_name'),
                  bio: text(),
                  createdAt: integer('created_at', { mode: 'timestamp' }),
                });
            "#,
        )];

        let columns = collect_expected_db_columns(&artifacts);
        let user_columns = columns.get("users").expect("users columns");
        // Explicit builder names win over the TS keys.
        assert!(user_columns.contains("full_name"), "got {user_columns:?}");
        assert!(user_columns.contains("created_at"), "got {user_columns:?}");
        assert!(
            !user_columns.contains("fullname"),
            "TS property key must not be the expected column when the builder names one, got {user_columns:?}"
        );
        assert!(!user_columns.contains("createdat"), "got {user_columns:?}");
        // The no-argument builder form falls back to the property key
        // (Drizzle derives the column name from the key there).
        assert!(user_columns.contains("bio"), "got {user_columns:?}");
        assert!(user_columns.contains("id"), "got {user_columns:?}");
    }

    #[test]
    fn prisma_relation_fields_are_not_expected_columns() {
        let artifacts = vec![artifact(
            "prisma/schema.prisma",
            r#"
                model Post {
                  id       String @id
                  title    String
                  author   User   @relation(fields: [authorId], references: [id])
                  authorId String
                  tags     Tag[]
                }
            "#,
        )];

        let columns = collect_expected_db_columns(&artifacts);
        let post_columns = columns.get("post").expect("post columns");
        assert!(post_columns.contains("id"));
        assert!(post_columns.contains("title"));
        // The FK scalar is a real column; the relation object and list are not.
        assert!(post_columns.contains("authorid"));
        assert!(
            !post_columns.contains("author"),
            "relation object field must be excluded, got {post_columns:?}"
        );
        assert!(
            !post_columns.contains("tags"),
            "list relation field must be excluded, got {post_columns:?}"
        );
    }
}
