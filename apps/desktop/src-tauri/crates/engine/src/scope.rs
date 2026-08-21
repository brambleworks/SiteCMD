//! Revisioned routes and check families that future scans should observe.
//!
//! Wire and hosted limits constrain connected scope, not local scans.

use crate::route::{canonical_path, CanonicalRoute};
use serde::{Deserialize, Serialize};

/// The wire-format bound on the scope resource (protocol spec). A schema
/// limit, not a scanning promise.
pub const SCOPE_WIRE_LIMIT: usize = 5_000;

/// The hosted scanner's version-1 route ceiling. Per-plan connected-scope
/// allowances are set at or under this; nothing here invents one.
pub const HOSTED_SCOPE_CEILING: usize = 100;

/// A scope that cannot be stored, with the numbers the message needs. Every
/// variant names its bound: a refusal that does not say the limit leaves the
/// user guessing which routes to drop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum ScopeError {
    /// More routes than the resource can hold.
    ExceedsWireLimit { requested: usize, limit: usize },
    /// More routes than the plan will scan (`422 scope_exceeds_plan`).
    ExceedsPlan { requested: usize, cap: usize },
    /// Scope has no routes.
    Empty,
}

impl ScopeError {
    /// Stable user-facing message shared by every client.
    pub fn message(&self) -> String {
        match self {
            Self::ExceedsWireLimit { requested, limit } => format!(
                "A scan scope holds at most {limit} routes; this one has {requested}. Remove routes to continue."
            ),
            Self::ExceedsPlan { requested, cap } => format!(
                "Connected monitoring covers {cap} routes on this plan; this scope has {requested}. Trim the scope or move to a plan with a larger allowance."
            ),
            Self::Empty => {
                "A scan scope needs at least one route. The entry page is always included.".into()
            }
        }
    }
}

/// Ordered monitored routes with the required entry route first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanScope {
    pub entry_route: String,
    pub routes: Vec<CanonicalRoute>,
    pub check_families: Vec<String>,
}

impl ScanScope {
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// Split plan-covered and overflow routes deterministically, entry first.
    pub fn effective_prefix(&self, cap: usize) -> (Vec<CanonicalRoute>, Vec<CanonicalRoute>) {
        let mut ordered = self.routes.clone();
        ordered.sort_by(|a, b| {
            let entry_rank = |route: &CanonicalRoute| u8::from(route.route != self.entry_route);
            entry_rank(a)
                .cmp(&entry_rank(b))
                .then_with(|| a.route.cmp(&b.route))
        });
        let kept = ordered.len().min(cap);
        let overflow = ordered.split_off(kept);
        (ordered, overflow)
    }
}

/// Canonicalize selected routes, add the entry route, deduplicate, and apply cap.
pub fn build_scope(
    entry_url: &url::Url,
    selected_routes: &[String],
    check_families: Vec<String>,
    cap: Option<usize>,
) -> Result<ScanScope, ScopeError> {
    let entry_route = canonical_path(entry_url.path());
    let mut routes: Vec<CanonicalRoute> = vec![CanonicalRoute::new(entry_route.clone(), false)];
    for selected in selected_routes {
        let route = canonical_path(selected);
        if routes.iter().any(|existing| existing.route == route) {
            continue;
        }
        routes.push(CanonicalRoute::new(route, false));
    }

    if routes.len() > SCOPE_WIRE_LIMIT {
        return Err(ScopeError::ExceedsWireLimit {
            requested: routes.len(),
            limit: SCOPE_WIRE_LIMIT,
        });
    }
    if let Some(cap) = cap {
        if routes.len() > cap {
            return Err(ScopeError::ExceedsPlan {
                requested: routes.len(),
                cap,
            });
        }
    }

    Ok(ScanScope {
        entry_route,
        routes,
        check_families,
    })
}

/// Return producible check families from the capability manifest.
pub fn engine_check_families() -> Vec<String> {
    let mut families: Vec<String> = crate::manifest::registry::entries()
        .filter(|entry| entry.lane != crate::manifest::HostedLane::Unsupported)
        .filter_map(|entry| entry.check.split('.').next())
        .map(str::to_string)
        .collect();
    families.sort_unstable();
    families.dedup();
    families
}

/// Resolve a scope's routes back to absolute URLs against the environment.
/// Scanners take URLs; storage keeps routes, because a stored URL would
/// carry the scheme and host twice and drift when either changes.
pub fn scope_urls(entry_url: &url::Url, routes: &[CanonicalRoute]) -> Vec<String> {
    routes
        .iter()
        .filter_map(|route| entry_url.join(&route.route).ok())
        .map(String::from)
        .collect()
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
