//! Stylesheet bodies shared by the pages of one scan execution.
//!
//! Sites link the same site-wide stylesheets from every page, so a twenty-page
//! scan used to download the same four files twenty times. One cache is built
//! per scan execution, shared by that execution's pages, and dropped with it,
//! so a later scan always re-reads the site.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use crate::constants::{MAX_STYLESHEET_CACHE_BYTES, MAX_STYLESHEET_CACHE_ENTRIES};

/// One settled fetch outcome. `None` records a stylesheet the origin answered
/// but refused to serve usably (a non-success status, or a body outside the
/// size and time limits): repeating that request would produce the same
/// answer. Outcomes that never got an answer (timeout, transport failure) are
/// deliberately not stored, so a later page retries them instead of inheriting
/// one page's blip as incomplete coverage.
pub type CachedStylesheet = Option<String>;

#[derive(Default)]
struct CacheState {
    entries: HashMap<String, CachedStylesheet>,
    /// Insertion order, so the budget evicts the oldest stylesheet first.
    order: VecDeque<String>,
    bytes: usize,
}

/// Byte- and entry-bounded stylesheet store keyed by resolved URL.
pub struct StylesheetCache {
    state: Mutex<CacheState>,
    max_entries: usize,
    max_bytes: usize,
}

impl StylesheetCache {
    /// Build the cache one scan execution shares across its pages.
    pub fn new() -> Self {
        Self::with_limits(MAX_STYLESHEET_CACHE_ENTRIES, MAX_STYLESHEET_CACHE_BYTES)
    }

    /// Build a cache with explicit bounds. Tests use this to prove eviction
    /// without allocating the production budget.
    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            state: Mutex::new(CacheState::default()),
            max_entries,
            max_bytes,
        }
    }

    /// Return the recorded outcome for `url`, or `None` when this execution
    /// has not fetched it yet. The inner `Option` is the outcome itself.
    pub fn get(&self, url: &str) -> Option<CachedStylesheet> {
        let state = self.state.lock().ok()?;
        state.entries.get(url).cloned()
    }

    /// Record the outcome for `url`. The first outcome for a URL wins, so a
    /// page cannot overwrite what an earlier page in the same execution
    /// observed. A body larger than the whole budget is not stored, and
    /// oversubscribing either bound evicts the oldest entries first.
    pub fn insert(&self, url: &str, value: CachedStylesheet) {
        if self.max_entries == 0 {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.entries.contains_key(url) {
            return;
        }
        let cost = value.as_ref().map_or(0, |css| css.len());
        if cost > self.max_bytes {
            return;
        }
        while state.order.len() + 1 > self.max_entries || state.bytes + cost > self.max_bytes {
            let Some(oldest) = state.order.pop_front() else {
                break;
            };
            if let Some(evicted) = state.entries.remove(&oldest) {
                state.bytes = state
                    .bytes
                    .saturating_sub(evicted.as_ref().map_or(0, |css| css.len()));
            }
        }
        state.bytes += cost;
        state.order.push_back(url.to_string());
        state.entries.insert(url.to_string(), value);
    }

    /// Number of stylesheets currently held. Used by tests and diagnostics.
    pub fn len(&self) -> usize {
        self.state
            .lock()
            .map(|state| state.order.len())
            .unwrap_or(0)
    }

    /// Total cached body bytes currently held.
    pub fn bytes(&self) -> usize {
        self.state.lock().map(|state| state.bytes).unwrap_or(0)
    }

    /// `true` when this execution has recorded no stylesheet yet.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for StylesheetCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "stylesheet_cache_tests.rs"]
mod tests;
