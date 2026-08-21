use super::*;

pub(in crate::core::code_scan) fn collect_expected_prisma_migration_names(
    project_paths_lower: &[String],
) -> Vec<String> {
    let mut migrations = HashSet::new();

    for path in project_paths_lower {
        if !path.ends_with("/migration.sql") {
            continue;
        }

        let Some((_, tail)) = path.split_once("prisma/migrations/") else {
            continue;
        };
        let Some(migration_name) = tail.split('/').next() else {
            continue;
        };
        if !migration_name.is_empty() {
            migrations.insert(migration_name.to_string());
        }
    }

    let mut collected = migrations.into_iter().collect::<Vec<_>>();
    collected.sort();
    collected
}

pub(in crate::core::code_scan) fn collect_expected_drizzle_migration_names(
    artifacts: &[TextArtifact],
    project_paths_lower: &[String],
) -> Vec<String> {
    let mut migrations = HashSet::new();

    for artifact in artifacts {
        let relative_lower = artifact.relative_path.to_ascii_lowercase();
        if !relative_lower.ends_with("drizzle/meta/_journal.json") {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(&artifact.content) else {
            continue;
        };
        let Some(entries) = value.get("entries").and_then(Value::as_array) else {
            continue;
        };
        for entry in entries {
            let Some(tag) = entry.get("tag").and_then(Value::as_str) else {
                continue;
            };
            let normalized = tag.trim().to_ascii_lowercase();
            if !normalized.is_empty() {
                migrations.insert(normalized);
            }
        }
    }

    for path in project_paths_lower {
        if !path.ends_with(".sql") || path.contains("/meta/") {
            continue;
        }
        let Some((_, tail)) = path.split_once("drizzle/") else {
            continue;
        };
        let Some(file_name) = tail.split('/').next_back() else {
            continue;
        };
        let Some(stem) = file_name.strip_suffix(".sql") else {
            continue;
        };
        let normalized = stem.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            migrations.insert(normalized);
        }
    }

    let mut collected = migrations.into_iter().collect::<Vec<_>>();
    collected.sort();
    collected
}
