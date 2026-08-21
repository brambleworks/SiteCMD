use super::*;

pub(in crate::core::code_scan) fn collect_registry_config(
    root: &Path,
    manifests: &[PackageManifest],
) -> RegistryConfig {
    let mut config = RegistryConfig {
        default_hosts: HashSet::from([
            "registry.npmjs.org".to_string(),
            "registry.yarnpkg.com".to_string(),
            "registry.npmjs.com".to_string(),
        ]),
        scope_hosts: HashMap::new(),
    };

    let mut candidate_paths = vec![root.join(".npmrc")];
    for manifest in manifests {
        if let Some(parent) = manifest.absolute_path.parent() {
            candidate_paths.push(parent.join(".npmrc"));
        }
    }

    for path in candidate_paths {
        let Some(content) = crate::updates::read_dependency_file(&path) else {
            continue;
        };
        merge_registry_config(&mut config, &content);
    }

    config
}

pub(in crate::core::code_scan) fn merge_registry_config(
    config: &mut RegistryConfig,
    content: &str,
) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let Some(host) = host_from_url(value) else {
            continue;
        };

        if key.eq_ignore_ascii_case("registry") {
            config.default_hosts.insert(host);
            continue;
        }

        if let Some(scope) = key.strip_suffix(":registry") {
            config
                .scope_hosts
                .entry(scope.to_ascii_lowercase())
                .or_default()
                .insert(host);
        }
    }
}

pub(in crate::core::code_scan) fn allowed_registry_hosts_for_dependency(
    dependency: &str,
    config: &RegistryConfig,
) -> HashSet<String> {
    if dependency.starts_with('@') {
        if let Some((scope, _)) = dependency.split_once('/') {
            if let Some(hosts) = config.scope_hosts.get(&scope.to_ascii_lowercase()) {
                return hosts.clone();
            }
        }
    }
    config.default_hosts.clone()
}

pub(in crate::core::code_scan) fn collect_lockfile_registry_hosts(
    manifest_dir: &Path,
) -> HashMap<String, String> {
    if let Some(entries) = parse_package_lock_registry_hosts(manifest_dir) {
        return entries;
    }
    if let Some(entries) = parse_yarn_lock_registry_hosts(manifest_dir) {
        return entries;
    }
    parse_pnpm_lock_registry_hosts(manifest_dir).unwrap_or_default()
}

pub(in crate::core::code_scan) fn parse_package_lock_registry_hosts(
    dir: &Path,
) -> Option<HashMap<String, String>> {
    let content = crate::updates::read_dependency_file(&dir.join("package-lock.json"))?;
    let lock: Value = serde_json::from_str(&content).ok()?;
    let version = lock
        .get("lockfileVersion")
        .and_then(|value| value.as_u64())
        .unwrap_or(1);

    if version >= 2 {
        let packages = lock.get("packages")?.as_object()?;
        let mut hosts = HashMap::new();
        for (key, info) in packages {
            let Some(name) = key.strip_prefix("node_modules/") else {
                continue;
            };
            let slash_count = name.matches('/').count();
            let is_scoped = name.starts_with('@');
            if (is_scoped && slash_count > 1) || (!is_scoped && slash_count > 0) {
                continue;
            }
            let Some(resolved) = info.get("resolved").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(host) = host_from_url(resolved) else {
                continue;
            };
            hosts.insert(name.to_ascii_lowercase(), host);
        }
        return Some(hosts);
    }

    let dependencies = lock.get("dependencies")?.as_object()?;
    let mut hosts = HashMap::new();
    for (name, info) in dependencies {
        let Some(resolved) = info.get("resolved").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(host) = host_from_url(resolved) else {
            continue;
        };
        hosts.insert(name.to_ascii_lowercase(), host);
    }
    Some(hosts)
}

/// Parse registry hosts from Yarn Classic lockfile `resolved` entries.
pub(in crate::core::code_scan) fn parse_yarn_lock_registry_hosts(
    dir: &Path,
) -> Option<HashMap<String, String>> {
    let content = crate::updates::read_dependency_file(&dir.join("yarn.lock"))?;
    let mut hosts = HashMap::new();
    let mut current_names: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            current_names.clear();
            continue;
        }

        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.contains('@') {
            current_names.clear();
            let header = trimmed.trim_end_matches(':').trim_matches('"');
            for spec in header.split(", ") {
                let spec = spec.trim().trim_matches('"');
                if let Some(name) = extract_package_name_from_spec(spec) {
                    current_names.push(name.to_ascii_lowercase());
                }
            }
            continue;
        }

        if (line.starts_with("  ") || line.starts_with('\t')) && trimmed.starts_with("resolved ") {
            let resolved = trimmed
                .trim_start_matches("resolved ")
                .trim_matches('"')
                .trim_matches('\'');
            let Some(host) = host_from_url(resolved) else {
                continue;
            };
            for name in &current_names {
                hosts.entry(name.clone()).or_insert_with(|| host.clone());
            }
            current_names.clear();
        }
    }

    if hosts.is_empty() {
        None
    } else {
        Some(hosts)
    }
}

pub(in crate::core::code_scan) fn extract_package_name_from_spec(spec: &str) -> Option<String> {
    if let Some(after_scope) = spec.strip_prefix('@') {
        if let Some(slash_pos) = after_scope.find('/') {
            let after_slash = &after_scope[slash_pos + 1..];
            if let Some(at_pos) = after_slash.find('@') {
                Some(format!("@{}", &after_scope[..slash_pos + 1 + at_pos]))
            } else {
                Some(format!("@{}", after_scope))
            }
        } else {
            None
        }
    } else {
        spec.find('@').map(|pos| spec[..pos].to_string())
    }
}

pub(in crate::core::code_scan) fn parse_pnpm_lock_registry_hosts(
    dir: &Path,
) -> Option<HashMap<String, String>> {
    let content = crate::updates::read_dependency_file(&dir.join("pnpm-lock.yaml"))?;
    let mut hosts = HashMap::new();
    let mut current_package: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if !line.starts_with(' ') && trimmed.ends_with(':') {
            current_package = None;
            continue;
        }

        if line.starts_with("  ") && trimmed.ends_with(':') {
            let key = trimmed
                .trim_end_matches(':')
                .trim_matches('"')
                .trim_matches('\'');
            if key.starts_with('/') || key.contains('@') {
                current_package = normalize_pnpm_package_key(key);
            }
            continue;
        }

        if trimmed.starts_with("tarball:") || trimmed.starts_with("resolution:") {
            let Some(package) = current_package.as_ref() else {
                continue;
            };
            let Some(url_start) = trimmed.find("http") else {
                continue;
            };
            let resolved = &trimmed[url_start..];
            let Some(host) = host_from_url(resolved) else {
                continue;
            };
            hosts.entry(package.clone()).or_insert(host);
        }
    }

    if hosts.is_empty() {
        None
    } else {
        Some(hosts)
    }
}

pub(in crate::core::code_scan) fn normalize_pnpm_package_key(key: &str) -> Option<String> {
    let trimmed = key.trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('@') {
        let segments = rest.split('/').collect::<Vec<_>>();
        if segments.len() < 2 {
            return None;
        }
        // Strip pnpm's version suffix from scoped keys such as
        // `@babel/core@7.24.0` before matching declarations.
        let name = segments[1].split('@').next().filter(|v| !v.is_empty())?;
        return Some(format!("@{}/{}", segments[0], name).to_ascii_lowercase());
    }
    trimmed
        .split('@')
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

pub(in crate::core::code_scan) fn host_from_url(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches(',');
    let url = url::Url::parse(normalized).ok()?;
    Some(url.host_str()?.to_ascii_lowercase())
}

pub(in crate::core::code_scan) fn dependency_spec_uses_remote_url(spec: &str) -> bool {
    let normalized = spec.trim().to_ascii_lowercase();
    normalized.starts_with("git+")
        || normalized.starts_with("github:")
        || normalized.starts_with("git://")
        || normalized.starts_with("https://")
        || normalized.starts_with("http://")
}

pub(in crate::core::code_scan) fn format_registry_host_list(hosts: &HashSet<String>) -> String {
    let mut values = hosts.iter().cloned().collect::<Vec<_>>();
    values.sort();
    values.join(", ")
}

#[cfg(test)]
mod tests {
    use super::normalize_pnpm_package_key;

    #[test]
    fn pnpm_keys_strip_version_suffixes_for_scoped_and_unscoped_packages() {
        assert_eq!(
            normalize_pnpm_package_key("@babel/core@7.24.0").as_deref(),
            Some("@babel/core")
        );
        assert_eq!(
            normalize_pnpm_package_key("/@babel/core/7.24.0").as_deref(),
            Some("@babel/core")
        );
        assert_eq!(
            normalize_pnpm_package_key("lodash@4.17.21").as_deref(),
            Some("lodash")
        );
        assert_eq!(normalize_pnpm_package_key("@scope").as_deref(), None);
    }
}
