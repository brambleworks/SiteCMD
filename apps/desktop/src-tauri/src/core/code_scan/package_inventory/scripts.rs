use super::*;

/// Return declared packages invoked by package scripts so CLI-only dependencies
/// are not reported unused.
pub(in crate::core::code_scan) fn collect_script_referenced_packages(
    manifest_content: &str,
    declared: &[String],
) -> HashSet<String> {
    let Ok(json) = serde_json::from_str::<Value>(manifest_content) else {
        return HashSet::new();
    };
    let Some(scripts) = json.get("scripts").and_then(|value| value.as_object()) else {
        return HashSet::new();
    };

    let mut combined = String::new();
    for script in scripts.values() {
        if let Some(command) = script.as_str() {
            combined.push(' ');
            combined.push_str(command);
        }
    }
    if combined.is_empty() {
        return HashSet::new();
    }

    let mut found = HashSet::new();
    for dependency in declared {
        if appears_as_script_token(&combined, dependency) {
            found.insert(dependency.clone());
        }
    }
    for (bin, package) in SCRIPT_BIN_PACKAGES {
        if !declared.iter().any(|dependency| dependency == package) {
            continue;
        }
        if appears_as_script_token(&combined, bin) {
            found.insert((*package).to_string());
        }
    }
    found
}

/// Executables package scripts invoke, mapped to the package that installs
/// them. Most tools ship a bin named after the package, which the name scan
/// above already covers; these are the ones whose bin and package names differ
/// or that are easy to miss.
static SCRIPT_BIN_PACKAGES: &[(&str, &str)] = &[
    ("nest", "@nestjs/cli"),
    ("tsc", "typescript"),
    ("prisma", "prisma"),
    ("vitest", "vitest"),
    ("jest", "jest"),
    ("eslint", "eslint"),
    ("prettier", "prettier"),
    ("next", "next"),
    ("astro", "astro"),
    ("vite", "vite"),
    ("tsx", "tsx"),
    ("ts-node", "ts-node"),
    ("playwright", "playwright"),
    ("playwright", "@playwright/test"),
];

/// Match package names as script tokens rather than substrings.
pub(in crate::core::code_scan) fn appears_as_script_token(scripts: &str, package: &str) -> bool {
    let target = package.to_ascii_lowercase();
    let haystack = scripts.to_ascii_lowercase();
    let mut start = 0usize;
    while let Some(index) = haystack[start..].find(&target) {
        let abs = start + index;
        let end = abs + target.len();
        let before_ok = abs == 0
            || !haystack
                .as_bytes()
                .get(abs - 1)
                .map(|c| c.is_ascii_alphanumeric() || *c == b'-' || *c == b'_' || *c == b'@')
                .unwrap_or(false);
        let after_ok = end == haystack.len()
            || !haystack
                .as_bytes()
                .get(end)
                .map(|c| c.is_ascii_alphanumeric() || *c == b'-' || *c == b'_' || *c == b'/')
                .unwrap_or(false);
        if before_ok && after_ok {
            return true;
        }
        start = abs + 1;
        if start >= haystack.len() {
            break;
        }
    }
    false
}
