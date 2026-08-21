//! Portable check evaluation over an already-fetched [`PageContext`].
//! Synchronous checks have no transport or clock access and run identically natively and in wasm.

// The check-authoring surface mirrors the desktop's `crate::checks` imports
// (vocab, PageContext, the string-boundary and HTML-attribute helpers), so a
// check module moves between the trees without rewriting its paths.
pub use crate::page::{origin_with_port, PageContext};
pub use crate::probe::looks_like_html_shell;
pub use crate::vocab::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

pub mod accessibility;
pub mod compliance;
pub mod config;
pub mod html_attrs;
pub mod performance;
pub mod predeploy;
pub mod security;
pub mod seo;
mod text_boundaries;
pub use text_boundaries::{ceil_char_boundary, floor_char_boundary};

/// Trait for checks that only analyze already-fetched data (synchronous)
pub trait Check: Send + Sync {
    fn id(&self) -> &str;
    fn category(&self) -> ScanCategory;
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult>;
    fn skip_in_predeploy(&self) -> bool {
        false
    }
}
