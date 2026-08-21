//! Portable connected-payload decisions shared by the desktop and CLI.

use crate::db::ProducerIdentity;

/// Return the sequence an inspector may display without reserving it.
pub fn proposed_submission_sequence(producer: Option<&ProducerIdentity>) -> Result<i64, String> {
    producer
        .map(|identity| identity.last_submission_sequence)
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "connected submission sequence is exhausted".to_string())
}

/// Build a stable canonical bootstrap scope with the entry route first.
/// Preserve every authored route regardless of entitlement limits.
pub fn initial_scope_routes(
    submission: &sitecmd_engine::sync::DesktopSubmission,
    environment_url: &str,
) -> Vec<String> {
    let entry_route = url::Url::parse(environment_url)
        .map(|url| sitecmd_engine::route::canonical_path(url.path()))
        .unwrap_or_else(|_| String::from("/"));
    let mut routes: Vec<String> = submission
        .snapshots
        .web
        .as_ref()
        .map(|snapshot| snapshot.coverage.routes.clone())
        .unwrap_or_default();
    routes = routes
        .into_iter()
        .map(|route| sitecmd_engine::route::canonical_path(&route))
        .collect();
    routes.push(entry_route.clone());
    routes.sort_by(|left, right| {
        let entry_rank = |route: &String| u8::from(route != &entry_route);
        entry_rank(left)
            .cmp(&entry_rank(right))
            .then_with(|| left.cmp(right))
    });
    routes.dedup();
    routes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submission_with_routes(routes: &[&str]) -> sitecmd_engine::sync::DesktopSubmission {
        use sitecmd_engine::coverage::ScanCoverageKind;
        use sitecmd_engine::sync::{WebSnapshot, WebVersions, WireCoverage, WireExecutionProfile};
        let mut submission = sitecmd_engine::sync::DesktopSubmission::new("site_1", 1);
        submission.snapshots.web = Some(WebSnapshot {
            observed_at: 0,
            based_on_event_sequence: 0,
            versions: WebVersions {
                engine_release: String::new(),
                fingerprint_schema: 1,
                canonicalizer: 1,
                crawl_profile: 1,
            },
            manifest_digest: String::new(),
            evaluation_time: 0,
            execution_profile: WireExecutionProfile::default(),
            stack_facts: None,
            coverage: WireCoverage {
                kind: ScanCoverageKind::PageSet,
                complete: true,
                routes: routes.iter().map(|route| route.to_string()).collect(),
                checks: Vec::new(),
                exceptions: Vec::new(),
            },
            occurrences: Vec::new(),
            measurement_samples: Vec::new(),
        });
        submission
    }

    #[test]
    fn initial_scope_is_the_bootstrap_coverage_or_the_entry_route() {
        let scoped = initial_scope_routes(
            &submission_with_routes(&["/pricing", "/", "/blog"]),
            "https://example.com/",
        );
        assert_eq!(scoped, vec!["/", "/blog", "/pricing"]);

        let bare = initial_scope_routes(
            &sitecmd_engine::sync::DesktopSubmission::new("site", 1),
            "https://example.com/",
        );
        assert_eq!(bare, vec!["/"]);
    }

    #[test]
    fn initial_scope_uses_the_environment_urls_non_root_entry() {
        let scoped = initial_scope_routes(
            &sitecmd_engine::sync::DesktopSubmission::new("site", 1),
            "https://example.com/app/",
        );
        assert_eq!(scoped, vec!["/app/"]);
    }

    #[test]
    fn initial_scope_preserves_every_bootstrap_route_for_server_validation() {
        let many: Vec<String> = (0..150).map(|index| format!("/page-{index:03}")).collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        let scoped = initial_scope_routes(&submission_with_routes(&refs), "https://example.com/");
        assert_eq!(scoped.len(), 151);
        assert_eq!(scoped[0], "/");
        assert_eq!(scoped[1], "/page-000");
        assert_eq!(scoped[150], "/page-149");

        // With the entry listed, nothing is inserted and all authored routes
        // still survive.
        let mut listed: Vec<String> = (0..150).map(|index| format!("/p-{index:03}")).collect();
        listed.push("/".into());
        let refs: Vec<&str> = listed.iter().map(String::as_str).collect();
        let scoped = initial_scope_routes(&submission_with_routes(&refs), "https://example.com/");
        assert_eq!(scoped.len(), 151);
        assert_eq!(scoped[0], "/");
        assert_eq!(scoped[150], "/p-149");
    }
}
