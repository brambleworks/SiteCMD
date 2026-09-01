use crate::checks::{CheckResult, CheckStatus, Severity};
use crate::core::code_scan::CodeIssue;

pub fn normalize_check_results(results: &mut [CheckResult]) {
    for result in results {
        result.severity = normalized_web_issue_severity(result);
    }
}

pub fn normalize_code_issues(issues: &mut [CodeIssue]) {
    for issue in issues {
        issue.severity = normalized_code_issue_severity(issue);
    }
}

pub fn normalized_web_issue_severity(result: &CheckResult) -> Severity {
    if matches!(result.status, CheckStatus::Pass | CheckStatus::Skipped) {
        return Severity::Low;
    }

    let check_id = result.check_id.as_str();
    let severity = web_policy_severity(result).unwrap_or(result.severity);

    if severity == Severity::Critical && !web_check_allows_critical(check_id) {
        return Severity::High;
    }
    // A Warn is an advisory; it never carries Critical weight in the score.
    // Checks that mean Critical must Fail.
    if severity == Severity::Critical && matches!(result.status, CheckStatus::Warn) {
        return Severity::High;
    }
    severity
}

pub fn normalized_code_issue_severity(issue: &CodeIssue) -> Severity {
    let severity = policy_clamped_code_severity(issue);
    // Advisory rules describe recommendations rather than proven exposure, so
    // the central policy caps them at Medium.
    if is_advisory_rule(&issue.id) {
        cap_at_medium(severity)
    } else {
        severity
    }
}

/// Apply registry severity and the Critical eligibility clamp before the
/// advisory-class cap.
pub(crate) fn policy_clamped_code_severity(issue: &CodeIssue) -> Severity {
    let severity = code_policy_severity(issue).unwrap_or(issue.severity);
    if severity == Severity::Critical && !code_issue_allows_critical(&issue.id) {
        Severity::High
    } else {
        severity
    }
}

/// True when the issue's rule is registered as an advisory class.
fn is_advisory_rule(issue_id: &str) -> bool {
    crate::core::code_scan::registry::descriptor_for_issue_id(issue_id)
        .map(|descriptor| descriptor.class == crate::core::code_scan::registry::RuleClass::Advisory)
        .unwrap_or(false)
}

/// Clamp a severity down to Medium (Critical/High -> Medium; Medium/Low kept).
fn cap_at_medium(severity: Severity) -> Severity {
    match severity {
        Severity::Critical | Severity::High => Severity::Medium,
        other => other,
    }
}

fn web_policy_severity(result: &CheckResult) -> Option<Severity> {
    let check_id = result.check_id.as_str();
    Some(match check_id {
        // Preserve this check's graded Medium/Low branches.
        "polish.missing-og-tags" => result.severity,
        id if id.starts_with("polish.") => polish_signal_severity(id),
        id if id.starts_with("accessibility.axe.") => axe_violation_severity(result),

        "accessibility.aria_usage"
        | "accessibility.empty_headings"
        | "accessibility.focus_indicators"
        | "accessibility.form_labels"
        | "accessibility.iframe_title"
        | "accessibility.image_alt"
        | "accessibility.landmarks"
        | "accessibility.lang"
        | "accessibility.link_text"
        | "accessibility.skip_nav"
        | "accessibility.tabindex" => Severity::Medium,
        // Static markup establishes autoplay intent but not whether playback
        // occurs, its duration, or the controls present at runtime. The check
        // grades unmuted declarations Medium and muted motion review Low.
        "accessibility.autoplay" => result.severity,
        "accessibility.headings" => Severity::Low,
        // Zoom-disabling viewport tags block reading for low-vision users
        // (WCAG 1.4.4); a real barrier, not a nicety.
        "accessibility.viewport_zoom" => Severity::High,
        "accessibility.color_contrast_hints" | "accessibility.redundant_alt" => Severity::Low,

        "compliance.privacy_policy" => Severity::Medium,
        "compliance.consent_mode" | "compliance.cookie_consent" => Severity::Medium,
        "compliance.accessibility_statement"
        | "compliance.ccpa_notice"
        | "compliance.cookie_expiration"
        | "compliance.data_controller_contact"
        | "compliance.dnt_respect"
        | "compliance.form_consent"
        | "compliance.terms"
        | "compliance.trackers" => Severity::Low,

        "config.localhost_refs" => Severity::Medium,
        // Preserve source-only evidence without claiming rendered responsiveness.
        "config.responsive_design" => result.severity,
        "config.debug_mode" => result.severity,
        "config.dev_dependencies" => Severity::Low,
        "config.placeholder_content" | "config.www_redirect" => Severity::Medium,
        "config.analytics"
        | "config.console_logs"
        | "config.custom_404"
        | "config.deprecated_html"
        | "config.favicon"
        | "config.print_stylesheet"
        | "config.sitemap_in_robots"
        | "config.todo_comments"
        | "config.trailing_slash"
        | "config.web_manifest" => Severity::Low,

        // Both the preliminary HTTP probe and its browser-navigation replacement
        // are single samples, so TTFB never grades above Medium.
        "performance.ttfb" => Severity::Medium,
        "performance.cls" | "performance.fcp" | "performance.lcp" => {
            status_severity(result, Severity::Medium, Severity::High)
        }
        // PageSpeed Insights supplies real Lighthouse TBT; the lightweight
        // browser probe below intentionally has a different identity.
        "performance.tbt" => status_severity(result, Severity::Medium, Severity::High),
        "performance.long_task_blocking" => {
            status_severity(result, Severity::Low, Severity::Medium)
        }
        "performance.inline_css" | "performance.render_blocking" | "performance.unminified" => {
            result.severity
        }
        "performance.broken_images"
        | "performance.cache"
        | "performance.compression"
        | "performance.http2"
        | "performance.images.heavy" => Severity::Medium,
        // Threshold checks that grade Warn at the advisory tier and Fail at
        // the real-cost tier; a Warn must not count as a full Medium.
        "performance.dom_size" | "performance.third_party" => {
            status_severity(result, Severity::Low, Severity::Medium)
        }
        // A caching miss on immutable assets is a pure optimization.
        "performance.asset_caching" => Severity::Low,
        // Measured transfer weight and oversized HTML documents are direct
        // user-facing load costs; they escalate on Fail.
        "performance.asset_weight" | "performance.page_weight" => {
            status_severity(result, Severity::Medium, Severity::High)
        }
        "performance.fonts"
        | "performance.http_requests"
        | "performance.images"
        | "performance.images.dimensions"
        | "performance.images.format"
        | "performance.images.lazy"
        | "performance.preconnect"
        | "performance.redirect_chain" => status_severity(result, Severity::Low, Severity::Medium),

        // The certificate sub-checks each grade themselves: expiry and
        // hostname reach Critical on direct evidence, chain splits
        // definitive rejections from trust-store differences, and protocol
        // tops out at High.
        "security.ssl.expiry" | "security.ssl.hostname" | "security.ssl.chain" => {
            status_severity(result, Severity::High, Severity::Critical)
        }
        "security.ssl.protocol" => status_severity(result, Severity::Medium, Severity::High),
        // HTTP enforcement grades direct 2xx evidence High, ambiguous chains
        // Medium, and temporary/error outcomes Low. Preserve those branches.
        "security.https_enforcement" => result.severity,
        "security.insecure_form" => status_severity(result, Severity::Medium, Severity::Critical),
        // These grade themselves per branch. `security.env_leak` is a
        // localhost-preview heuristic (credential-shaped literal = High,
        // unresolved reference = Medium); the live-page secret checks retain
        // their own verified/review branch grading.
        "security.env_leak"
        | "security.vibe.env_exposure"
        | "security.vibe.exposed_keys"
        | "security.vibe.exposed_keys.public"
        | "security.vibe.hardcoded_secrets" => result.severity,
        "security.cors"
        | "security.cors_reflection"
        | "security.directory_listing"
        | "security.form_action_hijack"
        | "security.open_redirect" => status_severity(result, Severity::Medium, Severity::High),
        // Direct active mixed content on an HTTPS page is High; passive or
        // responsive candidates are Medium, and localhost preview references
        // are a Medium review. Preserve the producer's evidence-based branch.
        "security.mixed_content" => result.severity,
        // Local-preview references do not prove a map is publicly deployed.
        "security.source_maps" => result.severity,
        // CSP/HSTS author every branch deliberately (missing enforcement is
        // Medium; contextual hardening advisories are Low); keep that grading.
        "security.headers.csp" | "security.headers.hsts" => result.severity,
        "security.vibe.client_auth" | "security.vibe.csrf" => {
            status_severity(result, Severity::Medium, Severity::High)
        }
        // Cookie branches distinguish direct contract failures from
        // contextual missing-attribute reviews and redact live values.
        "security.cookies" => result.severity,
        id if id.starts_with("security.cookies.") => result.severity,
        "security.email_exposure" | "security.headers.cross_origin" | "security.security_txt" => {
            Severity::Low
        }
        // Severity comes from the worst OSV advisory found for the library.
        "security.vulnerable_libraries" => result.severity,
        // SPF/DMARC grade themselves via MX gating and record content; domain
        // expiry grades by days remaining; a dangling www CNAME grades the
        // observed availability failure without assuming provider claimability.
        "security.dns.spf"
        | "security.dns.dmarc"
        | "security.dns.dangling_cname"
        | "security.domain_expiry" => result.severity,
        "security.dns.caa" | "security.dns.dkim" | "security.dns.dnssec" | "security.dns.mx" => {
            Severity::Low
        }
        // The header checks grade each branch deliberately (missing header
        // advisories at Low, leaky values at Medium, absent protections at
        // Medium/High); flattening them inflated Warn advisories.
        "security.headers.permissions_policy"
        | "security.headers.referrer_policy"
        | "security.headers.x_content_type_options"
        | "security.headers.x_frame_options"
        | "security.sri" => result.severity,
        "security.server_info.server_header" | "security.server_info.x_powered_by" => Severity::Low,
        id if id.starts_with("security.exposed_files.") => result.severity,

        "seo.title" => status_severity(result, Severity::Medium, Severity::High),
        // Warn = charset declared past the 1024-byte window; Fail = no
        // declaration anywhere, which garbles non-ASCII text unpredictably.
        "seo.charset" => status_severity(result, Severity::Low, Severity::Medium),
        // Branches distinguish an empty document, an optional missing sitemap,
        // an unverified cross-origin declaration, and an inconclusive probe.
        "seo.sitemap" => result.severity,
        "seo.robots_txt" => result.severity,
        "seo.viewport" => result.severity,
        // The directive is direct evidence; intent is contextual. Root-entry
        // pages grade High, while interior pages grade Medium.
        "seo.noindex" => result.severity,
        // The check grades a redirect Medium and a timed reload Low.
        "seo.meta_refresh" => result.severity,
        "seo.broken_links" => broken_link_severity(result),
        "seo.canonical_mismatch" => canonical_mismatch_severity(result),
        "seo.meta_conflicts" | "seo.meta_robots_conflicts" => meta_conflict_severity(result),
        "seo.broken_external_links"
        | "seo.duplicate_description"
        | "seo.duplicate_description_across_pages"
        | "seo.duplicate_meta"
        | "seo.duplicate_title"
        | "seo.duplicate_title_across_pages"
        | "seo.image_alt"
        | "seo.js_only_content"
        | "seo.meta_description"
        | "seo.og_image_relative"
        | "seo.og_image_status" => Severity::Medium,
        // Missing self-canonicals are contextual; the producer grades the
        // presence-only advisory Low and richer mismatch checks separately.
        "seo.canonical" | "seo.open_graph" | "seo.structured_data" => result.severity,
        "seo.headings.h1" => Severity::Low,
        // Warn = thin-ish page (advisory), Fail = near-empty body.
        "seo.thin_content" => status_severity(result, Severity::Low, Severity::Medium),
        "seo.ai_crawler_blocking" => result.severity,
        // Emitted severity varies: invalid JSON is Medium, recommended-only
        // property gaps are Low.
        "seo.structured_data.invalid" | "seo.structured_data.incomplete" => result.severity,
        // Cross-page (session-level) findings from core::session_analysis.
        "seo.canonical_loop" | "seo.noindex_in_sitemap" => Severity::Medium,
        "seo.duplicate_h1" | "seo.hreflang_reciprocity" | "seo.orphan_pages" => Severity::Low,
        "seo.citation_meta"
        | "seo.content_freshness"
        | "seo.faq_schema"
        | "seo.headings.hierarchy"
        | "seo.hreflang"
        | "seo.link_count"
        | "seo.llms_txt"
        | "seo.organization_identity"
        | "seo.page_speed_hints"
        | "seo.semantic_html"
        | "seo.sitemap_freshness"
        | "seo.source_citations"
        | "seo.temporary_redirect"
        | "seo.twitter_cards"
        | "seo.url_structure" => Severity::Low,

        _ => return None,
    })
}

/// Returns the registry's severity override for a code check. `None` preserves
/// the emitted severity for passthrough and uncatalogued checks.
fn code_policy_severity(issue: &CodeIssue) -> Option<Severity> {
    let descriptor = crate::core::code_scan::registry::descriptor_for_issue_id(&issue.id)?;
    Some(descriptor.policy_severity.unwrap_or(issue.severity))
}

pub fn polish_signal_severity(check_id: &str) -> Severity {
    let signal_id = check_id.strip_prefix("polish.").unwrap_or(check_id);
    match signal_id {
        "form-accessibility" | "button-vs-clickable-div" | "js-errors" => Severity::Medium,
        // The Polish signal observes only a sourceMappingURL reference; it
        // does not request the map. Keep it below the verified-exposure tier.
        "source-maps-production" => Severity::Medium,
        _ => Severity::Low,
    }
}

fn axe_violation_severity(result: &CheckResult) -> Severity {
    match result.severity {
        Severity::Critical | Severity::High => Severity::High,
        Severity::Medium => Severity::Medium,
        Severity::Low => Severity::Low,
    }
}

fn broken_link_severity(result: &CheckResult) -> Severity {
    if raw_array_len(result, "broken").unwrap_or(1) >= 3 {
        Severity::High
    } else {
        Severity::Medium
    }
}

fn canonical_mismatch_severity(result: &CheckResult) -> Severity {
    let _ = result;
    // A differing canonical target is directly observed but can be deliberate
    // for duplicate or syndicated content. Until equivalence, target state,
    // and owner intent are verified, a cross-host target is not a High defect.
    Severity::Medium
}

fn meta_conflict_severity(result: &CheckResult) -> Severity {
    match result.severity {
        Severity::Critical | Severity::High => Severity::High,
        _ => Severity::Medium,
    }
}

fn status_severity(result: &CheckResult, warn: Severity, fail: Severity) -> Severity {
    match result.status {
        CheckStatus::Fail => fail,
        CheckStatus::Warn => warn,
        CheckStatus::Pass | CheckStatus::Skipped => Severity::Low,
    }
}

fn raw_array_len(result: &CheckResult, key: &str) -> Option<usize> {
    result
        .raw_data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_array())
        .map(Vec::len)
}

fn web_check_allows_critical(check_id: &str) -> bool {
    matches!(
        check_id,
        "security.ssl.expiry"
            | "security.ssl.hostname"
            | "security.ssl.chain"
            | "security.https_enforcement"
            | "security.insecure_form"
    ) || check_id.starts_with("security.exposed_files")
}

fn code_issue_allows_critical(issue_id: &str) -> bool {
    crate::core::code_scan::registry::descriptor_for_issue_id(issue_id)
        .map(|descriptor| descriptor.allows_critical)
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "severity_policy_tests.rs"]
mod tests;
