use super::*;

pub(in crate::core::code_scan) fn collect_package_manifests(
    project_files: &[ProjectFile],
) -> Vec<PackageManifest> {
    project_files
        .iter()
        .filter_map(|file| {
            if file.absolute_path.file_name()?.to_string_lossy() != "package.json" {
                return None;
            }
            if file.size > 250_000 {
                return None;
            }
            let content =
                read_project_file(file, 250_000).and_then(|bytes| String::from_utf8(bytes).ok())?;
            let Ok(json) = serde_json::from_str::<Value>(&content) else {
                return None;
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

            Some(PackageManifest {
                absolute_path: file.absolute_path.clone(),
                relative_path: file.relative_path.clone(),
                content,
                package_name,
                dependencies,
                installed_dependencies,
                local_dependencies,
                dependency_specs,
            })
        })
        .collect()
}
