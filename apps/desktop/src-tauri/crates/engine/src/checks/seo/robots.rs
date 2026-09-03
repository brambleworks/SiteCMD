//! Portable robots.txt parsing and `seo.robots_txt` verdicts.
//! Runtimes own fetching; this module owns classification and grading.

use crate::checks::seo::robots_directives::has_sitemap_directive;
use crate::checks::{
    looks_like_html_shell, CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity,
};
use crate::probe::ProbeOutcome;

/// Outcome of the shared per-scan robots.txt fetch. Several checks inspect
/// robots.txt with different semantics (content, missing, unreachable), so
/// the memo keeps the full outcome rather than just a body.
#[derive(Debug)]
pub enum RobotsTxtFetch {
    /// 2xx response with a readable non-HTML-shell body.
    Found { body: String },
    /// Reachable but non-2xx.
    Status(u16),
    /// A 2xx response whose body is an HTML page rather than a robots.txt: a
    /// catch-all route answered for the path. The endpoint responded, so this
    /// is not a network failure and re-running will not change it.
    HtmlShell,
    /// Network-level failure or an unreadable body.
    Error(String),
}

/// The canonical robots.txt probe URL for a scanned page.
pub fn robots_txt_url(page_url: &url::Url) -> String {
    format!("{}/robots.txt", crate::checks::origin_with_port(page_url))
}

/// Classify a completed robots.txt probe. A read failure (timeout mid-body
/// or exceeding the probe size cap) must not collapse into an empty `Found`.
/// A genuinely empty 2xx body still resolves to `Found`.
pub fn robots_fetch_from_probe(outcome: ProbeOutcome) -> RobotsTxtFetch {
    match outcome {
        ProbeOutcome::Response(response) if (200..300).contains(&response.status) => {
            let content_type = response
                .content_type
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase();
            match response.body {
                Some(body) => {
                    if looks_like_html_shell(&content_type, &body.text) {
                        RobotsTxtFetch::HtmlShell
                    } else {
                        RobotsTxtFetch::Found { body: body.text }
                    }
                }
                None => RobotsTxtFetch::Error(
                    "could not read /robots.txt (the response body was unavailable)".into(),
                ),
            }
        }
        ProbeOutcome::Response(response) => RobotsTxtFetch::Status(response.status),
        ProbeOutcome::Failure(failure) => RobotsTxtFetch::Error(failure.detail),
    }
}

/// One robots.txt group: the user agents naming it and whether its own
/// rules disallow the site root (with no same-group `Allow: /` re-opening
/// it). Agents are lowercased.
pub struct RobotsGroup {
    pub agents: Vec<String>,
    pub root_disallow: bool,
    pub has_allow_rule: bool,
    pub blocks_root: bool,
}

/// Parse crawler groups, including consecutive `User-agent:` lines that share
/// the following rule block.
pub fn robots_groups(body: &str) -> Vec<RobotsGroup> {
    let mut groups: Vec<RobotsGroup> = Vec::new();
    let mut current_agents: Vec<String> = Vec::new();
    let mut seen_rule = false;
    let mut disallow_root = false;
    let mut has_allow_rule = false;

    for raw in body.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let Some((directive, value)) = lower.split_once(':') else {
            continue;
        };
        let directive = directive.trim();
        let value = value.trim();
        if directive == "user-agent" {
            // A user-agent line after a rule line begins a new group;
            // close out the group we are leaving first.
            if seen_rule {
                groups.push(RobotsGroup {
                    agents: std::mem::take(&mut current_agents),
                    root_disallow: disallow_root,
                    has_allow_rule,
                    blocks_root: disallow_root && !has_allow_rule,
                });
                seen_rule = false;
                disallow_root = false;
                has_allow_rule = false;
            }
            if !value.is_empty() {
                current_agents.push(value.to_string());
            }
            continue;
        }
        if current_agents.is_empty() {
            continue;
        }
        seen_rule = true;
        if directive == "disallow" {
            if matches!(value, "/" | "/*") {
                disallow_root = true;
            }
        } else if directive == "allow" && !value.is_empty() {
            // Any explicit exception means `Disallow: /` is not a
            // literal whole-site block. This conservative summary does
            // not attempt a full path-pattern crawl simulation.
            has_allow_rule = true;
        }
    }
    if !current_agents.is_empty() {
        groups.push(RobotsGroup {
            agents: current_agents,
            root_disallow: disallow_root,
            has_allow_rule,
            blocks_root: disallow_root && !has_allow_rule,
        });
    }
    groups
}

/// Whether the `User-agent: *` group itself disallows the entire site.
pub fn wildcard_blocks_entire_site(body: &str) -> bool {
    let groups = robots_groups(body);
    let wildcard_groups: Vec<_> = groups
        .iter()
        .filter(|group| group.agents.iter().any(|agent| agent == "*"))
        .collect();
    wildcard_groups.iter().any(|group| group.root_disallow)
        && !wildcard_groups.iter().any(|group| group.has_allow_rule)
}

/// Grade the `seo.robots_txt` outcome from the shared fetch. Missing files
/// and Sitemap directives are optional and are not treated as defects; only
/// a root-wide wildcard block with no `Allow` exception warns.
pub fn evaluate_robots_txt(fetch: &RobotsTxtFetch) -> Vec<CheckResult> {
    match fetch {
        RobotsTxtFetch::Found { body } => {
            // A wildcard group is a fallback. More-specific user-agent
            // groups can override it, so do not call this "all crawlers."
            let wildcard_default_block = wildcard_blocks_entire_site(body);

            // Check for sitemap reference
            let has_sitemap = has_sitemap_directive(body);

            vec![CheckResult {
                check_id: "seo.robots_txt".into(),
                category: ScanCategory::Seo,
                title: if wildcard_default_block {
                    "robots.txt broadly blocks crawling by default".into()
                } else {
                    "Robots.txt policy observed".into()
                },
                description: if wildcard_default_block {
                    "The `User-agent: *` fallback group has `Disallow: /` (or `/*`) and no nonempty `Allow` exception. Compliant crawlers that select this wildcard group are instructed not to crawl the site. A crawler with a more-specific matching group may use that group instead, so this check does not claim every crawler is blocked.".into()
                } else if has_sitemap {
                    "robots.txt is present, includes a Sitemap directive, and has no root-wide wildcard block without an Allow exception. This bounded check does not validate every path pattern, crawler-specific group, sitemap target, robots syntax edge case, CDN policy, or actual crawler behavior.".into()
                } else {
                    "robots.txt is present with no root-wide wildcard block without an Allow exception. No Sitemap directive was observed, but that directive is an optional discovery hint and its absence does not block crawling or indexing.".into()
                },
                status: if wildcard_default_block {
                    CheckStatus::Warn
                } else {
                    CheckStatus::Pass
                },
                severity: if wildcard_default_block {
                    Severity::High
                } else {
                    Severity::Low
                },
                fix_prompt: None,
                manual_fix: if wildcard_default_block {
                    Some("Confirm whether the wildcard block is intentional for this environment. If public crawling is intended, remove the root-wide `Disallow: /`/`/*`, narrow it to the sensitive paths that should not be crawled, or add explicit crawler groups only when policy requires them. Preserve authentication for private data because robots.txt is not access control. Test representative important URLs with the crawler groups and edge controls that matter.".into())
                } else {
                    None
                },
                raw_data: Some(serde_json::json!({
                    "has_sitemap_directive": has_sitemap,
                    "wildcard_default_block": wildcard_default_block,
                    "specific_user_agent_overrides_evaluated": false,
                })),
                confidence: if wildcard_default_block {
                    IssueConfidence::NeedsReview
                } else {
                    IssueConfidence::High
                },
                confidence_reason: if wildcard_default_block {
                    Some("The wildcard rule is directly observed, but specific crawler groups, path-pattern precedence beyond this conservative summary, and edge-level access can change the effective result for a given crawler.".into())
                } else {
                    None
                },
                why_it_matters: if wildcard_default_block {
                    Some("For crawlers that fall back to the wildcard group, a root-wide disallow prevents normal crawling and can impair discovery or refresh. Intentional staging/private-site policies may make that the desired outcome.".into())
                } else {
                    None
                },
            }]
        }
        RobotsTxtFetch::Status(code) if matches!(*code, 404 | 410) => {
            let mut result = missing_robots_result();
            result.raw_data = Some(serde_json::json!({
                "robots_policy_evaluated": false,
                "status_code": code,
                "confirmed_missing": true,
            }));
            result.confidence = IssueConfidence::High;
            result.confidence_reason = None;
            vec![result]
        }
        RobotsTxtFetch::Status(code) => vec![robots_unavailable_result(
            Some(*code),
            "robots.txt returned a non-success response",
        )],
        RobotsTxtFetch::HtmlShell => vec![robots_html_shell_result()],
        RobotsTxtFetch::Error(_) => vec![robots_unavailable_result(
            None,
            "the robots.txt request failed",
        )],
    }
}

/// Result for a robots.txt path answered by an HTML catch-all route. The
/// endpoint responded, so the copy must not call this a failed request or ask
/// for a re-run.
pub fn robots_html_shell_result() -> CheckResult {
    CheckResult {
        check_id: "seo.robots_txt".into(),
        category: ScanCategory::Seo,
        title: "Robots.txt policy not evaluated".into(),
        description: "robots.txt answered with an HTML page (catch-all rewrite), not a robots.txt file, so no rules could be parsed and no robots policy conclusion was made. Serve the file as text/plain from that path if crawl rules are intended.".into(),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "robots_policy_evaluated": false,
            "html_catch_all": true,
            "probe_conclusive": false,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: Some(
            "The response body is an HTML document, so the endpoint served a page rather than a robots.txt."
                .into(),
        ),
        why_it_matters: None,
    }
}

/// Result for a confirmed missing robots.txt response.
fn missing_robots_result() -> CheckResult {
    CheckResult {
        check_id: "seo.robots_txt".into(),
        category: ScanCategory::Seo,
        title: "No robots.txt file".into(),
        description: "robots.txt is absent. Under the standard missing-file behavior, that means no robots crawl restrictions; publishing the optional file is useful only when the operator needs crawl rules or a Sitemap discovery hint.".into(),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({"robots_policy_evaluated": false, "confirmed_missing": true})),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

fn robots_unavailable_result(status_code: Option<u16>, observation: &str) -> CheckResult {
    CheckResult {
        check_id: "seo.robots_txt".into(),
        category: ScanCategory::Seo,
        title: "Robots.txt policy not evaluated".into(),
        description: format!(
            "{}{}; no robots policy conclusion was made. Re-run when the endpoint is readable.",
            observation,
            status_code
                .map(|code| format!(" (HTTP {code})"))
                .unwrap_or_default()
        ),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "robots_policy_evaluated": false,
            "status_code": status_code,
            "probe_conclusive": false,
        })),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "The endpoint did not return a readable robots.txt body, so rules could not be parsed."
                .into(),
        ),
        why_it_matters: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ProbeBody, ProbeFailure, ProbeFailureClass, ProbeResponse};

    fn response(status: u16, content_type: &str, body: Option<&str>) -> ProbeOutcome {
        ProbeOutcome::Response(ProbeResponse {
            status,
            final_url: "https://example.com/robots.txt".into(),
            content_type: Some(content_type.to_string()),
            content_length: None,
            headers: Vec::new(),
            body: body.map(|text| ProbeBody {
                text: text.to_string(),
                bytes: text.len(),
                utf8_valid: true,
            }),
        })
    }

    #[test]
    fn ai_bot_block_is_not_a_site_wide_block() {
        let robots = "User-agent: GPTBot\nDisallow: /\n\nUser-agent: *\nDisallow:\n";
        assert!(!wildcard_blocks_entire_site(robots));
    }

    #[test]
    fn wildcard_disallow_root_is_a_site_wide_block() {
        assert!(wildcard_blocks_entire_site("User-agent: *\nDisallow: /\n"));
        // Grouped agents sharing the rule still count.
        assert!(wildcard_blocks_entire_site(
            "User-agent: GPTBot\nUser-agent: *\nDisallow: /\n"
        ));
    }

    #[test]
    fn allow_root_in_same_group_reopens_the_site() {
        assert!(!wildcard_blocks_entire_site(
            "User-agent: *\nDisallow: /\nAllow: /\n"
        ));
    }

    #[test]
    fn allow_exception_means_the_entire_site_is_not_blocked() {
        assert!(!wildcard_blocks_entire_site(
            "User-agent: *\nDisallow: /\nAllow: /public/\n"
        ));
    }

    #[test]
    fn repeated_wildcard_groups_are_combined_before_classifying_a_total_block() {
        assert!(!wildcard_blocks_entire_site(
            "User-agent: *\nDisallow: /\n\nUser-agent: *\nAllow: /public/\n"
        ));
    }

    #[test]
    fn parser_accepts_whitespace_around_directive_colons() {
        assert!(wildcard_blocks_entire_site(
            "User-agent : *\nDisallow : /\n"
        ));
    }

    #[test]
    fn wildcard_path_form_is_recognized_as_a_root_wide_block() {
        assert!(wildcard_blocks_entire_site("User-agent: *\nDisallow: /*\n"));
    }

    #[test]
    fn robots_groups_share_rules_across_consecutive_user_agents() {
        let robots =
            "User-agent: GPTBot\nUser-agent: CCBot\nDisallow: /\n\nUser-agent: *\nDisallow:\n";
        let groups = robots_groups(robots);
        let blocked: Vec<&str> = groups
            .iter()
            .filter(|g| g.blocks_root)
            .flat_map(|g| g.agents.iter().map(String::as_str))
            .collect();
        assert_eq!(blocked, vec!["gptbot", "ccbot"]);
    }

    #[test]
    fn robots_groups_folder_disallow_does_not_block_root() {
        let robots = "User-agent: GPTBot\nDisallow: /private/\n";
        let groups = robots_groups(robots);
        assert!(groups.iter().all(|g| !g.blocks_root));
    }

    #[test]
    fn missing_robots_txt_is_not_scored_as_a_defect() {
        let result = missing_robots_result();
        assert_eq!(result.status, CheckStatus::Skipped);
        assert_eq!(result.severity, Severity::Low);
        assert!(result.description.contains("no robots crawl restrictions"));
    }

    #[test]
    fn folder_scoped_disallow_is_not_a_site_wide_block() {
        assert!(!wildcard_blocks_entire_site(
            "User-agent: *\nDisallow: /admin/\nDisallow: /private/\n"
        ));
    }

    #[test]
    fn blocks_all_fix_names_the_disallow_line() {
        let results = evaluate_robots_txt(&RobotsTxtFetch::Found {
            body: "User-agent: *\nDisallow: /\n".into(),
        });
        assert_eq!(results[0].status, CheckStatus::Warn);
        let fix = results[0].manual_fix.as_deref().unwrap_or("");
        assert!(
            fix.contains("Disallow: /") && fix.to_ascii_lowercase().contains("remove"),
            "fix must name the offending line: {fix}"
        );
    }

    #[test]
    fn missing_sitemap_directive_is_not_described_as_a_crawl_block() {
        let result = evaluate_robots_txt(&RobotsTxtFetch::Found {
            body: "User-agent: *\nAllow: /\n".into(),
        })
        .remove(0);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.manual_fix.is_none());
        assert!(result.description.contains("optional discovery hint"));
        assert!(!result.description.contains("entire site"));
    }

    #[test]
    fn successful_probe_with_a_text_body_classifies_as_found() {
        let outcome = response(200, "text/plain", Some("User-agent: *\nDisallow:\n"));
        let RobotsTxtFetch::Found { body } = robots_fetch_from_probe(outcome) else {
            panic!("expected Found for a normal robots.txt body");
        };
        assert!(body.contains("User-agent"));
        // A genuinely empty robots.txt (complete read, empty body) stays
        // Found; only failed reads are inconclusive.
        assert!(matches!(
            robots_fetch_from_probe(response(200, "text/plain", Some(""))),
            RobotsTxtFetch::Found { .. }
        ));
    }

    #[test]
    fn html_catch_all_rewrite_is_its_own_outcome_not_an_empty_robots() {
        let outcome = response(200, "text/html; charset=utf-8", Some("<!doctype html>"));
        assert!(matches!(
            robots_fetch_from_probe(outcome),
            RobotsTxtFetch::HtmlShell
        ));
    }

    #[test]
    fn an_html_catch_all_robots_policy_row_does_not_claim_a_failed_request() {
        let rows = evaluate_robots_txt(&RobotsTxtFetch::HtmlShell);
        assert_eq!(rows[0].status, CheckStatus::Skipped);
        assert!(
            !rows[0].description.contains("request failed"),
            "{}",
            rows[0].description
        );
        assert!(
            rows[0].description.contains("catch-all rewrite"),
            "{}",
            rows[0].description
        );
        assert_eq!(rows[0].raw_data.as_ref().unwrap()["html_catch_all"], true);
    }

    #[test]
    fn non_success_status_classifies_as_status() {
        assert!(matches!(
            robots_fetch_from_probe(response(404, "text/html", None)),
            RobotsTxtFetch::Status(404)
        ));
    }

    #[test]
    fn probe_failure_classifies_as_error_never_found() {
        // A timed-out or capped body read must not become Found { "" }.
        let outcome = ProbeOutcome::Failure(ProbeFailure {
            class: ProbeFailureClass::BodyCapExceeded,
            detail: "body exceeded cap".into(),
        });
        assert!(matches!(
            robots_fetch_from_probe(outcome),
            RobotsTxtFetch::Error(_)
        ));
    }

    #[test]
    fn robots_url_preserves_non_default_ports() {
        let url = url::Url::parse("http://localhost:8080/deep/page").unwrap();
        assert_eq!(robots_txt_url(&url), "http://localhost:8080/robots.txt");
    }
}
