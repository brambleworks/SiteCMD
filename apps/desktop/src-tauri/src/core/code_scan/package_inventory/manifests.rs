use super::*;

pub(in crate::core::code_scan) fn collect_package_manifests(
    project_files: &[ProjectFile],
    text_budget: &mut ScanTextBudget<'_>,
) -> Result<Vec<PackageManifest>, CodeScanError> {
    let mut manifests = Vec::new();
    for file in project_files {
        text_budget.check_cancelled()?;
        if file
            .absolute_path
            .file_name()
            .is_none_or(|name| name != "package.json")
            || file.size > 250_000
        {
            continue;
        }
        let Some(content) = text_budget.read_project_file(file, 250_000)? else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Value>(&content) else {
            continue;
        };

        let package_name = json
            .get("name")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());

        let mut dependencies = HashSet::new();
        let mut installed_dependencies = HashSet::new();
        let mut local_dependencies = HashSet::new();
        let mut dependency_specs = HashMap::new();
        for field in [
            "dependencies",
            "devDependencies",
            "peerDependencies",
            "optionalDependencies",
        ] {
            let Some(table) = json.get(field).and_then(Value::as_object) else {
                continue;
            };
            let installs = matches!(field, "dependencies" | "devDependencies");
            for (key, value) in table {
                let normalized = key.to_ascii_lowercase();
                dependencies.insert(normalized.clone());
                if installs {
                    installed_dependencies.insert(normalized.clone());
                }
                if let Some(spec) = value.as_str() {
                    dependency_specs.insert(normalized.clone(), spec.trim().to_string());
                }
                if dependency_spec_is_local(value) {
                    local_dependencies.insert(normalized);
                }
            }
        }

        manifests.push(PackageManifest {
            absolute_path: file.absolute_path.clone(),
            relative_path: file.relative_path.clone(),
            content,
            package_name,
            dependencies,
            installed_dependencies,
            local_dependencies,
            dependency_specs,
        });
    }
    Ok(manifests)
}
