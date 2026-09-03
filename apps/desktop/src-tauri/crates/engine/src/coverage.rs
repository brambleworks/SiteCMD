//! Pair-precise coverage for route and check verdicts.
//!
//! Pass, fail, and warn prove execution. Skipped checks and incomplete
//! set-level analysis become explicit exceptions. `covers` is authoritative.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::vocab::CheckStatus;

/// The registry's family prefixes: checks whose real ids are named at run
/// time (`accessibility.axe.<rule>`, `security.exposed_files.<path>`).
static FAMILY_PREFIXES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    crate::manifest::registry::entries()
        .filter(|entry| entry.family)
        .map(|entry| entry.check)
        .collect()
});

/// Returns a check's family prefix or its exact id. Family execution claims
/// dynamic members even when a fixed violation no longer emits an id; explicit
/// member exceptions still preserve inconclusive results.
pub fn claim_key(check_id: &str) -> &str {
    FAMILY_PREFIXES
        .iter()
        .filter(|prefix| check_id.starts_with(**prefix) && check_id.len() > prefix.len())
        // Longest match, so a family nested inside another resolves to the
        // more specific one.
        .max_by_key(|prefix| prefix.len())
        .copied()
        .unwrap_or(check_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "snake_case")]
pub enum ScanCoverageKind {
    Site,
    PageSet,
    Page,
    Project,
    CheckSet,
    RuleSet,
}

impl ScanCoverageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::PageSet => "page_set",
            Self::Page => "page",
            Self::Project => "project",
            Self::CheckSet => "check_set",
            Self::RuleSet => "rule_set",
        }
    }

    /// Whether the kind observes routes. Code kinds observe a project tree,
    /// which has no routes to key pairs on, so their pairs are the check
    /// alone.
    pub fn is_route_scoped(self) -> bool {
        matches!(
            self,
            Self::Page | Self::PageSet | Self::Site | Self::CheckSet
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "snake_case")]
pub enum CoverageExceptionReason {
    CheckSkipped,
    SessionIncomplete,
}

impl CoverageExceptionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CheckSkipped => "check_skipped",
            Self::SessionIncomplete => "session_incomplete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "camelCase")]
pub struct CoverageException {
    pub route: Option<String>,
    pub checks_not_run: Vec<String>,
    pub reason: CoverageExceptionReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "camelCase")]
pub struct ScanCoverageManifest {
    pub kind: ScanCoverageKind,
    pub successful: bool,
    #[serde(default)]
    pub page_urls: Vec<String>,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub exceptions: Vec<CoverageException>,
}

/// One check's outcome at one location: the raw material coverage is derived
/// from, so that no caller can assemble a claim out of what it INTENDED to
/// run.
#[derive(Debug, Clone, Copy)]
pub struct CheckOutcome<'a> {
    /// `None` for a producer with no routes (a code scan).
    pub route: Option<&'a str>,
    pub check_id: &'a str,
    pub status: CheckStatus,
}

/// What the run's claims are about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimBasis {
    /// Each route stands on its own: a check proven on one route says nothing
    /// about another, and nothing about a route that was never visited.
    PerRoute,
    /// The claims are about the route set as a whole, so an incomplete set
    /// proves none of them.
    RouteSet { complete: bool },
}

fn same_route(a: &str, b: &str) -> bool {
    a == b
}

impl ScanCoverageManifest {
    /// Derive claims and skipped-pair exceptions from observed outcomes.
    pub fn derive(
        kind: ScanCoverageKind,
        page_urls: Vec<String>,
        outcomes: &[CheckOutcome<'_>],
        basis: ClaimBasis,
    ) -> Self {
        let mut checks: BTreeSet<&str> = BTreeSet::new();
        let mut skipped: BTreeMap<Option<&str>, BTreeSet<&str>> = BTreeMap::new();
        for outcome in outcomes {
            checks.insert(claim_key(outcome.check_id));
            if outcome.status == CheckStatus::Skipped {
                skipped
                    .entry(outcome.route)
                    .or_default()
                    .insert(outcome.check_id);
            }
        }

        let mut exceptions = Vec::new();
        if basis == (ClaimBasis::RouteSet { complete: false }) && !checks.is_empty() {
            exceptions.push(CoverageException {
                route: None,
                checks_not_run: checks.iter().map(|check| (*check).to_string()).collect(),
                reason: CoverageExceptionReason::SessionIncomplete,
            });
        }
        exceptions.extend(
            skipped
                .into_iter()
                .map(|(route, checks)| CoverageException {
                    route: route.map(str::to_string),
                    checks_not_run: checks.into_iter().map(str::to_string).collect(),
                    reason: CoverageExceptionReason::CheckSkipped,
                }),
        );

        Self {
            kind,
            successful: true,
            page_urls,
            checks: checks.into_iter().map(str::to_string).collect(),
            exceptions,
        }
    }

    /// Declare complete coverage for finding-only producers whose registered
    /// rules run over every scanned file.
    pub fn declared(kind: ScanCoverageKind, page_urls: Vec<String>, checks: Vec<String>) -> Self {
        Self {
            kind,
            successful: true,
            page_urls,
            checks,
            exceptions: Vec::new(),
        }
    }

    /// A run that is still in flight or proved nothing. Claims nothing, so it
    /// resolves nothing.
    pub fn unproven(kind: ScanCoverageKind, page_urls: Vec<String>) -> Self {
        Self {
            kind,
            successful: false,
            page_urls,
            checks: Vec::new(),
            exceptions: Vec::new(),
        }
    }

    /// Whether this run proved the `(route, check)` pair.
    ///
    /// `route: None` asks about an observation with no route of its own (a
    /// site-level finding), which one claimed, unexcepted route is enough to
    /// prove.
    pub fn covers(&self, route: Option<&str>, check_id: &str) -> bool {
        if !self.successful || !self.claims_check(check_id) {
            return false;
        }
        if self.excepted_on(None, check_id) {
            return false;
        }
        if !self.kind.is_route_scoped() {
            return true;
        }
        match route {
            Some(route) => self.claims_route(route) && !self.excepted_on(Some(route), check_id),
            None => self
                .page_urls
                .iter()
                .any(|claimed| !self.excepted_on(Some(claimed), check_id)),
        }
    }

    /// The routes that bound what this manifest can cover, or `None` when the
    /// manifest observes no routes and any route may be covered.
    ///
    /// A reader may narrow its candidate rows to these routes plus the
    /// routeless observations before asking [`covers`](Self::covers), which
    /// stays authoritative: a route outside the bound is refused by `covers`
    /// anyway, so the narrowing only avoids reading rows that would be
    /// rejected.
    pub fn route_bound(&self) -> Option<&[String]> {
        self.kind
            .is_route_scoped()
            .then_some(self.page_urls.as_slice())
    }

    fn claims_check(&self, check_id: &str) -> bool {
        let key = claim_key(check_id);
        self.checks
            .iter()
            .any(|claimed| claimed == check_id || claimed == key)
    }

    fn claims_route(&self, route: &str) -> bool {
        self.page_urls
            .iter()
            .any(|claimed| same_route(claimed, route))
    }

    /// Test global and route-specific exceptions against exact or family IDs.
    fn excepted_on(&self, route: Option<&str>, check_id: &str) -> bool {
        let key = claim_key(check_id);
        self.exceptions.iter().any(|exception| {
            let applies = match (&exception.route, route) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some(excepted), Some(route)) => same_route(excepted, route),
            };
            applies
                && exception
                    .checks_not_run
                    .iter()
                    .any(|excepted| excepted == check_id || excepted == key)
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if matches!(
            self.kind,
            ScanCoverageKind::CheckSet | ScanCoverageKind::RuleSet
        ) && self.checks.is_empty()
        {
            return Err(format!(
                "{} coverage requires an explicit check set",
                self.kind.as_str()
            ));
        }
        if matches!(
            self.kind,
            ScanCoverageKind::Page | ScanCoverageKind::PageSet
        ) && self.page_urls.is_empty()
        {
            return Err(format!(
                "{} coverage requires at least one page URL",
                self.kind.as_str()
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "coverage_tests.rs"]
mod tests;
