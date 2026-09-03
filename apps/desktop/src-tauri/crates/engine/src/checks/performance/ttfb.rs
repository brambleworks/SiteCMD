//! Shared TTFB grading for runtime-supplied, vantage-dependent samples.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

pub const CHECK_ID: &str = "performance.ttfb";
pub const TITLE: &str = "Time to First Byte (TTFB)";

/// web.dev-aligned thresholds for one graded TTFB value, which is the fastest
/// of however many samples the vantage took.
fn grade_ttfb(ttfb_ms: u64) -> (CheckStatus, &'static str, String) {
    if ttfb_ms < 200 {
        (
            CheckStatus::Pass,
            TITLE,
            format!(
                "Excellent TTFB: {}ms. Server responds very quickly.",
                ttfb_ms
            ),
        )
    } else if ttfb_ms <= 800 {
        (
            CheckStatus::Pass,
            TITLE,
            format!("Good TTFB: {}ms.", ttfb_ms),
        )
    } else if ttfb_ms <= 1800 {
        (
            CheckStatus::Warn,
            "Server response over 800ms (TTFB)",
            format!("Slow TTFB: {}ms. Reduce server response time.", ttfb_ms),
        )
    } else {
        (
            CheckStatus::Fail,
            "Server response over 1.8s (TTFB)",
            format!(
                "Very slow TTFB: {}ms. This significantly impacts page load time.",
                ttfb_ms
            ),
        )
    }
}

fn guidance_for_ttfb(ttfb_ms: u64) -> Option<(&'static str, &'static str)> {
    if ttfb_ms <= 800 {
        return None;
    }
    Some((
        "Confirm the result with at least three scans, then isolate the delay in this order:\n\
         1. **Cold start.** Compare the first request with an immediate repeat. If only the first is slow, tune the serverless/container startup path or keep the service warm where the platform supports it.\n\
         2. **Origin and cache behavior.** Inspect CDN and server cache headers. Cache public HTML where safe, but do not edge-cache personalized responses without the correct cache key and privacy controls.\n\
         3. **Database work.** Use query logging or APM to find slow request-path queries, then index, batch, or cache the measured bottleneck.\n\
         4. **Synchronous third-party work.** Cache or batch required calls and defer only work that is not needed to authorize or construct the response. Never move an authentication or authorization decision into browser code.",
        "A slow first byte consumes the page's loading budget before the browser can begin rendering the response, making a good Largest Contentful Paint (LCP) harder to achieve. It does not directly determine layout stability or interaction latency.",
    ))
}

/// Grade one runtime-supplied TTFB sample. `measurement_source` names the
/// vantage that produced it (the desktop's HTTP probe, a Workers fetch) and
/// rides in raw_data so a sample never pretends to be placeless.
pub fn evaluate_ttfb(ttfb_ms: u64, measurement_source: &str) -> Vec<CheckResult> {
    evaluate_ttfb_samples(&[ttfb_ms], measurement_source)
}

/// Grade a repeated measurement of the same origin by its fastest sample.
///
/// Every sample includes whatever the network and the origin's accept queue
/// did to that one request, so the slowest sample is an upper bound on server
/// time, never a measurement of it: a cold connection, a TCP retransmit, or a
/// request queued behind other in-flight requests all inflate it. The minimum
/// is the closest observation to the server's own response time, so it is what
/// gets graded, and every sample is published for the reader.
pub fn evaluate_ttfb_samples(samples: &[u64], measurement_source: &str) -> Vec<CheckResult> {
    let Some(&ttfb_ms) = samples.iter().min() else {
        return ttfb_unavailable("no timing sample was taken");
    };
    let (status, title, description) = grade_ttfb(ttfb_ms);
    let guidance = guidance_for_ttfb(ttfb_ms);
    let sample_count = samples.len();

    vec![CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Performance,
        title: title.into(),
        description,
        status,
        // However many samples were taken, they all come from one vantage
        // point on one network path, so this never grades higher than Medium.
        severity: Severity::Medium,
        fix_prompt: guidance.map(|(manual_fix, _)| manual_fix.to_string()),
        manual_fix: guidance.map(|(manual_fix, _)| manual_fix.to_string()),
        raw_data: Some(if sample_count > 1 {
            serde_json::json!({
                "ttfb_ms": ttfb_ms,
                "measurement_source": measurement_source,
                "samples_ms": samples,
                "sample_count": sample_count,
                "graded_sample": "fastest",
            })
        } else {
            serde_json::json!({
                "ttfb_ms": ttfb_ms,
                "measurement_source": measurement_source,
            })
        }),
        confidence: if ttfb_ms >= 800 {
            IssueConfidence::NeedsReview
        } else {
            IssueConfidence::High
        },
        confidence_reason: if ttfb_ms < 800 {
            None
        } else if sample_count > 1 {
            Some(format!(
                "Fastest of {sample_count} requests from one vantage point; network path and origin load still ride along with every sample. Re-run the scan to confirm."
            ))
        } else {
            Some(
                "Measured from a single request; a cold start or cache miss can inflate one-off samples. Re-run the scan to confirm."
                    .into(),
            )
        },
        why_it_matters: guidance.map(|(_, why_it_matters)| why_it_matters.to_string()),
    }]
}

/// The sample never arrived: the timing request itself failed, so there is
/// no number to grade and no claim to make.
pub fn ttfb_unavailable(detail: &str) -> Vec<CheckResult> {
    let detail = crate::log_sanitizer::bounded_issue_evidence(detail);
    vec![CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Performance,
        title: TITLE.into(),
        description: format!("Could not measure TTFB: {}", detail),
        status: CheckStatus::Skipped,
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_follow_web_dev_guidance() {
        // Good <= 800ms, needs improvement <= 1800ms, poor above.
        assert_eq!(grade_ttfb(700).0, CheckStatus::Pass);
        assert_eq!(grade_ttfb(800).0, CheckStatus::Pass);
        assert_eq!(grade_ttfb(900).0, CheckStatus::Warn);
        assert_eq!(grade_ttfb(1800).0, CheckStatus::Warn);
        assert_eq!(grade_ttfb(1900).0, CheckStatus::Fail);
    }

    #[test]
    fn slow_ttfb_guidance_only_claims_the_metrics_it_directly_affects() {
        let (manual_fix, why_it_matters) = guidance_for_ttfb(900).expect("slow guidance");
        assert!(!manual_fix.contains("move the call to the client"));
        assert!(!why_it_matters.contains("every other Core Web Vital"));
        assert!(why_it_matters.contains("LCP"));
        assert!(guidance_for_ttfb(800).is_none());
    }

    #[test]
    fn a_sample_grades_with_its_vantage_recorded() {
        let results = evaluate_ttfb(150, "http_probe");
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].check_id, "performance.ttfb");
        let raw = results[0].raw_data.as_ref().expect("raw data");
        assert_eq!(raw["ttfb_ms"], 150);
        assert_eq!(raw["measurement_source"], "http_probe");
    }

    #[test]
    fn a_slow_sample_carries_single_sample_review_confidence() {
        let results = evaluate_ttfb(2500, "http_probe");
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0].manual_fix.is_some());
    }

    #[test]
    fn repeated_samples_are_graded_by_the_fastest_one() {
        // A first request that waits on connection setup, a retransmit, or the
        // origin's accept queue is an upper bound, not server time. Grading the
        // slow sample called visityourteam.com "1658ms" where three fresh
        // requests measured 93 to 225 ms.
        let results = evaluate_ttfb_samples(&[1658, 118], "http_probe");
        assert_eq!(results[0].status, CheckStatus::Pass);
        let raw = results[0].raw_data.as_ref().expect("raw data");
        assert_eq!(raw["ttfb_ms"], 118);
        assert_eq!(raw["samples_ms"], serde_json::json!([1658, 118]));
        assert_eq!(raw["sample_count"], 2);
        assert_eq!(raw["graded_sample"], "fastest");
    }

    #[test]
    fn a_consistently_slow_origin_is_still_graded_slow() {
        let results = evaluate_ttfb_samples(&[2400, 2200], "http_probe");
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(
            results[0]
                .confidence_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("Fastest of 2 requests")),
            "{:?}",
            results[0].confidence_reason
        );
    }

    #[test]
    fn a_single_sample_keeps_its_original_evidence_shape() {
        // The browser-navigation and hosted vantages supply exactly one
        // sample; their raw_data must not grow keys that describe repetition.
        let raw = evaluate_ttfb(150, "http_probe")[0]
            .raw_data
            .clone()
            .expect("raw data");
        assert_eq!(
            raw,
            serde_json::json!({"ttfb_ms": 150, "measurement_source": "http_probe"})
        );
    }

    #[test]
    fn an_empty_sample_set_reports_no_measurement() {
        let results = evaluate_ttfb_samples(&[], "http_probe");
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[test]
    fn a_failed_measurement_makes_no_claim() {
        let results = ttfb_unavailable("connection reset by peer");
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("Could not measure TTFB"));
    }
}
