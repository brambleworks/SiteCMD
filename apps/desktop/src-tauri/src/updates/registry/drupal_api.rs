//! Public Drupal.org client for recommended versions and project status.
//! The parser does not emit release-staleness signals.

use crate::updates::types::{
    classify_update, Ecosystem, InstalledPackage, PackageUpdate, UpdateType,
};
use reqwest::Client;

const CONCURRENCY_LIMIT: usize = 5;

/// Return Drupal updates plus whether any fetch was unobserved.
pub async fn check_updates(packages: &[InstalledPackage]) -> (Vec<PackageUpdate>, bool) {
    // Skip unknown versions before acquiring permits.
    let checkable: Vec<InstalledPackage> = packages
        .iter()
        .filter(|pkg| pkg.version != "unknown")
        .cloned()
        .collect();

    let fan_out =
        super::concurrency::check_registry_updates(&checkable, CONCURRENCY_LIMIT, |pkg| {
            let client = crate::http_client::client().clone();
            async move { fetch_latest(&client, &pkg.name, &pkg.version, &pkg.source).await }
        })
        .await;
    (fan_out.results, fan_out.failed > 0)
}

/// Strip the `drupal/` composer-style prefix to get the bare module name
/// the Drupal.org update API expects.
pub(crate) fn module_name_from_package(name: &str) -> &str {
    name.trim_start_matches("drupal/")
}

/// Identify one release block by Drupal's authoritative `Security update` term.
pub(crate) fn is_security_release(release_block: &str) -> bool {
    release_block.contains("Security update")
}

/// Maps `unsupported` and `revoked` project statuses to deprecation text.
pub(crate) fn project_status_deprecation(body: &str) -> Option<String> {
    for status in ["unsupported", "revoked"] {
        let marker = format!("<project_status>{}</project_status>", status);
        if body.contains(&marker) {
            return Some(format!(
                "The Drupal project is marked {} on drupal.org.",
                status
            ));
        }
    }
    None
}

/// Build an update or deprecation finding from Drupal release-history XML.
pub(crate) fn build_update_from_response(
    name: &str,
    current: &str,
    source: &str,
    body: &str,
) -> Option<PackageUpdate> {
    let deprecation_message = project_status_deprecation(body);
    let is_deprecated = deprecation_message.is_some();

    let latest = extract_latest_version(body).unwrap_or_default();
    let has_newer = !latest.is_empty() && latest != current;
    if !has_newer && !is_deprecated {
        return None;
    }

    let module = module_name_from_package(name);
    // Security status comes from relevant release blocks, never feed-wide boilerplate.
    // Deprecation without an installable update does not receive a security badge.
    let is_security = has_newer
        && ((!provably_different_branch(current, &latest)
            && recommended_release_block(body).is_some_and(is_security_release))
            || history_has_security_release_in_range(body, current, &latest)
            || installed_release_flagged_insecure(body, current));
    Some(PackageUpdate {
        name: name.to_string(),
        current_version: current.to_string(),
        // For a deprecated project with no newer stable release, anchor the
        // row to the installed version instead of implying an upgrade.
        latest_version: if latest.is_empty() {
            current.to_string()
        } else {
            latest.clone()
        },
        ecosystem: Ecosystem::Drupal,
        update_type: if has_newer {
            classify_update(current, &latest)
        } else {
            UpdateType::Unknown
        },
        is_security,
        advisory_severity: if is_security {
            Some("high".to_string())
        } else {
            None
        },
        advisory_url: if is_security {
            Some(format!(
                "https://www.drupal.org/project/{}/releases",
                module
            ))
        } else {
            None
        },
        source: source.to_string(),
        is_dev: false,
        is_deprecated,
        deprecation_message,
        ..Default::default()
    })
}

async fn fetch_latest(
    client: &Client,
    name: &str,
    current: &str,
    source: &str,
) -> Result<Option<PackageUpdate>, String> {
    let module = module_name_from_package(name);
    let url = format!(
        "https://updates.drupal.org/release-history/{}/current",
        module
    );
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if super::status_is_observed_absence(status) {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!(
            "Drupal.org returned status {} for {}",
            status, module
        ));
    }
    let body = crate::http_client::read_text_limited(
        resp,
        crate::constants::DRUPAL_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(build_update_from_response(name, current, source, &body))
}

/// The inner text of `<tag>...</tag>` within `fragment`, whitespace-trimmed.
/// Newline-agnostic, so it works on both pretty-printed and single-line feeds.
fn tag_content<'a>(fragment: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{}>", tag);
    let start = fragment.find(&open)? + open.len();
    let close = format!("</{}>", tag);
    let end = fragment[start..].find(&close)? + start;
    Some(fragment[start..end].trim())
}

/// A version we are willing to recommend: a stable release, never a
/// dev/alpha/beta/rc pre-release (production Drupal sites must not be pushed to
/// a pre-release).
fn is_stable_version(version: &str) -> bool {
    !version.contains("dev")
        && !version.contains("alpha")
        && !version.contains("beta")
        && !version.contains("rc")
}

/// A release Drupal has flagged as itself insecure (release type "Insecure").
/// We must never recommend upgrading TO such a release, so it is skipped when
/// picking the recommended version.
fn is_insecure_release(block: &str) -> bool {
    block.contains("<value>Insecure</value>") || block.contains("<term>Insecure</term>")
}

/// Return whether the exact installed release is marked `Insecure`.
pub(crate) fn installed_release_flagged_insecure(xml: &str, installed: &str) -> bool {
    release_blocks(xml)
        .any(|block| tag_content(block, "version") == Some(installed) && is_insecure_release(block))
}

/// Every `<release>` block in document order, independent of line formatting.
fn release_blocks(xml: &str) -> impl Iterator<Item = &str> + '_ {
    let mut rest = xml;
    std::iter::from_fn(move || {
        let open = rest.find("<release>")?;
        let after = &rest[open + "<release>".len()..];
        let close = after.find("</release>").unwrap_or(after.len());
        let block = &after[..close];
        rest = &after[close..];
        Some(block)
    })
}

/// The first `<release>` block whose `<version>` is a stable release that is
/// not flagged insecure. The feed lists releases newest-first, so this is the
/// recommended upgrade.
pub(crate) fn recommended_release_block(xml: &str) -> Option<&str> {
    release_blocks(xml).find(|block| {
        tag_content(block, "version").is_some_and(is_stable_version) && !is_insecure_release(block)
    })
}

/// Drupal contrib version ordered by scheme, then numeric components.
/// Security relevance remains restricted to the same scheme and branch.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DrupalVersion {
    Legacy(Vec<u64>),
    Semver(Vec<u64>),
}

impl DrupalVersion {
    /// Whether two versions use the same scheme and major branch.
    fn same_branch(&self, other: &DrupalVersion) -> bool {
        match (self, other) {
            (DrupalVersion::Legacy(a), DrupalVersion::Legacy(b))
            | (DrupalVersion::Semver(a), DrupalVersion::Semver(b)) => {
                a.first().copied().unwrap_or(0) == b.first().copied().unwrap_or(0)
            }
            _ => false,
        }
    }
}

/// Parse Drupal contrib or semantic versions; reject snapshots and prereleases.
fn parse_drupal_version(version: &str) -> Option<DrupalVersion> {
    if let Some((core, suffix)) = version.split_once(".x-") {
        if core.is_empty() || !core.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        return dotted_numeric_parts(suffix).map(DrupalVersion::Legacy);
    }
    dotted_numeric_parts(version).map(DrupalVersion::Semver)
}

/// Return true only when both versions parse and prove different branches.
fn provably_different_branch(a: &str, b: &str) -> bool {
    match (parse_drupal_version(a), parse_drupal_version(b)) {
        (Some(a), Some(b)) => !a.same_branch(&b),
        _ => false,
    }
}

/// `2.5.1` -> `[2, 5, 1]`, with trailing zeros trimmed so `2.5.0` == `2.5`
/// under the lexicographic Vec ordering. None for any non-numeric part.
fn dotted_numeric_parts(version: &str) -> Option<Vec<u64>> {
    if version.is_empty() {
        return None;
    }
    let mut parts = version
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()?;
    while parts.last() == Some(&0) {
        parts.pop();
    }
    Some(parts)
}

/// Detect security releases above the installed version on the same Drupal
/// scheme and branch. Unorderable versions are ignored rather than guessed.
pub(crate) fn history_has_security_release_in_range(
    xml: &str,
    installed: &str,
    recommended: &str,
) -> bool {
    let (Some(installed), Some(recommended)) = (
        parse_drupal_version(installed),
        parse_drupal_version(recommended),
    ) else {
        return false;
    };
    release_blocks(xml).any(|block| {
        is_security_release(block)
            && tag_content(block, "version")
                .and_then(parse_drupal_version)
                .is_some_and(|version| {
                    version.same_branch(&installed)
                        && version > installed
                        && (!recommended.same_branch(&installed) || version <= recommended)
                })
    })
}

pub(crate) fn extract_latest_version(xml: &str) -> Option<String> {
    recommended_release_block(xml)
        .and_then(|block| tag_content(block, "version"))
        .map(str::to_string)
}

#[cfg(test)]
#[path = "drupal_api_tests.rs"]
mod tests;
