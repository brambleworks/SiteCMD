use super::*;

pub(in crate::core::code_scan) fn suspicious_package_match(
    package_name: &str,
) -> Option<&'static str> {
    let target = package_name.trim().to_ascii_lowercase();
    if target.is_empty() {
        return None;
    }

    let base_target = package_basename(&target);

    // Exclude popular and curated near-neighbor packages from typo-squat matches.
    if KNOWN_GOOD_PACKAGE_BASENAMES.contains(&base_target.as_str())
        || POPULAR_PACKAGE_NAMES.iter().any(|popular| {
            let popular = popular.to_ascii_lowercase();
            popular == target || package_basename(&popular) == base_target
        })
    {
        return None;
    }

    // Compare scoped packages only with other scoped names.
    let target_is_scoped = target.starts_with('@');

    for candidate in POPULAR_PACKAGE_NAMES {
        let candidate_lower = candidate.to_ascii_lowercase();
        if target_is_scoped != candidate_lower.starts_with('@') {
            continue;
        }
        if candidate_lower == target || candidate_lower == base_target {
            continue;
        }

        let candidate_base = package_basename(&candidate_lower);
        if is_single_edit_away(&base_target, &candidate_base)
            || is_adjacent_swap_away(&base_target, &candidate_base)
        {
            return Some(candidate);
        }
    }

    None
}

pub(in crate::core::code_scan) fn package_basename(package_name: &str) -> String {
    if let Some((_, base)) = package_name.rsplit_once('/') {
        return base.to_string();
    }
    package_name.to_string()
}

pub(in crate::core::code_scan) fn is_single_edit_away(left: &str, right: &str) -> bool {
    if left == right {
        return false;
    }

    let left = left.as_bytes();
    let right = right.as_bytes();
    let len_diff = left.len().abs_diff(right.len());
    if len_diff > 1 {
        return false;
    }

    let mut i = 0;
    let mut j = 0;
    let mut edits = 0;

    while i < left.len() && j < right.len() {
        if left[i] == right[j] {
            i += 1;
            j += 1;
            continue;
        }

        edits += 1;
        if edits > 1 {
            return false;
        }

        if left.len() > right.len() {
            i += 1;
        } else if right.len() > left.len() {
            j += 1;
        } else {
            i += 1;
            j += 1;
        }
    }

    edits + (left.len() - i) + (right.len() - j) == 1
}

pub(in crate::core::code_scan) fn is_adjacent_swap_away(left: &str, right: &str) -> bool {
    if left.len() != right.len() || left == right {
        return false;
    }

    let left = left.as_bytes();
    let right = right.as_bytes();
    let mismatches = left
        .iter()
        .zip(right.iter())
        .enumerate()
        .filter_map(|(index, (a, b))| if a != b { Some(index) } else { None })
        .collect::<Vec<_>>();

    if mismatches.len() != 2 || mismatches[1] != mismatches[0] + 1 {
        return false;
    }

    let first = mismatches[0];
    left[first] == right[first + 1] && left[first + 1] == right[first]
}

pub(in crate::core::code_scan) fn has_named_dependency(
    dependencies: &HashSet<String>,
    candidates: &[&str],
) -> bool {
    candidates
        .iter()
        .any(|candidate| dependencies.contains(&candidate.to_ascii_lowercase()))
}

pub(in crate::core::code_scan) fn dependency_spec_is_local(value: &Value) -> bool {
    let Some(spec) = value.as_str() else {
        return false;
    };
    let normalized = spec.trim().to_ascii_lowercase();
    normalized.starts_with("workspace:")
        || normalized.starts_with("file:")
        || normalized.starts_with("link:")
        || normalized.starts_with("portal:")
        || normalized.starts_with('.')
        || normalized.starts_with('/')
}

pub(in crate::core::code_scan) fn should_ignore_unused_dependency(package_name: &str) -> bool {
    SUPPLY_CHAIN_UNUSED_IGNORE_EXACT
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(package_name))
        || SUPPLY_CHAIN_UNUSED_IGNORE_PREFIXES
            .iter()
            .any(|prefix| package_name.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legit_packages_near_popular_names_are_not_typosquats() {
        assert_eq!(suspicious_package_match("preact"), None);
        assert_eq!(suspicious_package_match("@astrojs/preact"), None);
        assert_eq!(suspicious_package_match("nuxt"), None);
        assert_eq!(suspicious_package_match("nest"), None);
        // A popular name is never a typo of another popular name.
        assert_eq!(suspicious_package_match("react"), None);
        assert_eq!(suspicious_package_match("next"), None);
    }

    #[test]
    fn genuine_typosquats_are_still_flagged() {
        assert_eq!(suspicious_package_match("raect"), Some("react")); // adjacent swap
        assert!(suspicious_package_match("expres").is_some()); // single deletion of express
    }

    #[test]
    fn scoped_first_party_packages_are_not_typosquats_of_unscoped_names() {
        assert_eq!(suspicious_package_match("@astrojs/prism"), None);
        assert_eq!(suspicious_package_match("@strapi/openapi"), None);
        assert!(suspicious_package_match("prism").is_some());
    }

    #[test]
    fn cli_and_plugin_dependencies_are_not_reported_as_unused_imports() {
        assert!(should_ignore_unused_dependency("@tauri-apps/cli"));
        assert!(should_ignore_unused_dependency("@vitest/coverage-v8"));
        assert!(should_ignore_unused_dependency("@size-limit/file"));
        assert!(should_ignore_unused_dependency("@cloudflare/workers-types"));
        assert!(should_ignore_unused_dependency("wrangler"));
        assert!(!should_ignore_unused_dependency("@sitecmd/runtime"));
    }
}
