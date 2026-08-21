//! Desktop re-exports for the portable scorer and live-score adapter.

pub use sitecmd_engine::scoring::calculator::*;

pub use crate::scoring::live::compute_current_score;
pub(crate) use crate::scoring::live::group_severity_penalty;

#[cfg(test)]
use crate::checks::Severity;

#[cfg(test)]
#[path = "calculator_current_score_tests.rs"]
mod current_score_tests;

#[cfg(test)]
#[path = "calculator_scan_tests.rs"]
mod tests;
