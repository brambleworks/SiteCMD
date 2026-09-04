use super::*;

pub(in crate::core::code_scan) fn collect_database_artifacts(
    project_files: &[ProjectFile],
    text_budget: &mut ScanTextBudget<'_>,
) -> Result<Vec<TextArtifact>, CodeScanError> {
    collect_text_artifacts(project_files, looks_like_database_artifact, text_budget)
}

fn looks_like_database_artifact(relative_path: &str, file_name: &str) -> bool {
    let relative_path = relative_path.to_ascii_lowercase();
    let file_name = file_name.to_ascii_lowercase();

    relative_path.ends_with("/schema.prisma")
        || relative_path == "schema.prisma"
        || relative_path.contains("/prisma/migrations/")
        || relative_path.starts_with("prisma/migrations/")
        || relative_path.contains("/supabase/migrations/")
        || relative_path.starts_with("supabase/migrations/")
        || relative_path.contains("/drizzle/")
        || relative_path.starts_with("drizzle/")
        || ((file_name == "schema.sql"
            || file_name == "schema.ts"
            || file_name == "schema.js"
            || file_name == "tables.ts"
            || file_name == "tables.js")
            && (relative_path.contains("/db/")
                || relative_path.contains("/database/")
                || relative_path.contains("/schema/")))
}

pub(in crate::core::code_scan) fn collect_deploy_config_files(
    project_files: &[ProjectFile],
    text_budget: &mut ScanTextBudget<'_>,
) -> Result<Vec<TextArtifact>, CodeScanError> {
    collect_text_artifacts(project_files, looks_like_deploy_config, text_budget)
}

fn looks_like_deploy_config(relative_path: &str, file_name: &str) -> bool {
    let relative_path = relative_path.to_ascii_lowercase();
    let file_name = file_name.to_ascii_lowercase();
    matches!(
        file_name.as_str(),
        "vercel.json"
            | "netlify.toml"
            | "wrangler.toml"
            | "fly.toml"
            | "railway.json"
            | "render.yaml"
            | "render.yml"
            | "docker-compose.yml"
            | "docker-compose.yaml"
            | "dockerfile"
    ) || relative_path.ends_with("/dockerfile")
}

pub(in crate::core::code_scan) fn collect_text_artifacts(
    project_files: &[ProjectFile],
    matches_artifact: fn(&str, &str) -> bool,
    text_budget: &mut ScanTextBudget<'_>,
) -> Result<Vec<TextArtifact>, CodeScanError> {
    let mut artifacts = Vec::new();
    for file in project_files {
        text_budget.check_cancelled()?;
        let Some(file_name) = file.absolute_path.file_name() else {
            continue;
        };
        if !matches_artifact(&file.relative_path, &file_name.to_string_lossy())
            || file.size > 250_000
        {
            continue;
        }
        let Some(content) = text_budget.read_project_file(file, 250_000)? else {
            continue;
        };
        artifacts.push(TextArtifact {
            absolute_path: file.absolute_path.clone(),
            relative_path: file.relative_path.clone(),
            content,
        });
    }
    Ok(artifacts)
}
