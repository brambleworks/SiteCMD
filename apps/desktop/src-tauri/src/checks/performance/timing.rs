//! Desktop TTFB measurement transport for portable grading.

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::performance::ttfb;

/// Measures TTFB by making a fresh request
pub struct TimingCheck;

#[async_trait::async_trait]
impl AsyncCheck for TimingCheck {
    fn id(&self) -> &str {
        "performance.ttfb"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let start = std::time::Instant::now();

        let resp = ctx
            .client
            .get(ctx.url.as_str())
            .timeout(crate::constants::CHECK_TIMEOUT)
            .send()
            .await;

        let ttfb_ms = start.elapsed().as_millis() as u64;

        match resp {
            Ok(_) => ttfb::evaluate_ttfb(ttfb_ms, "http_probe"),
            Err(error) => ttfb::ttfb_unavailable(&error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_matches_the_emitted_check_id() {
        assert_eq!(TimingCheck.id(), "performance.ttfb");
        assert_eq!(TimingCheck.id(), ttfb::CHECK_ID);
    }
}
