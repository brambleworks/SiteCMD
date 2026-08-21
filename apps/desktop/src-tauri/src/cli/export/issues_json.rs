use crate::cli::export::labels::{category_display_name, category_label, verify_hint};
use crate::cli::impact;
use crate::core::scanner::ScanResult;

pub(super) fn build_issues_json(result: &ScanResult) -> Result<String, String> {
    let ranked = impact::rank_issues(&result.issues, result.detected_stack.as_ref());
    let total_recoverable: u32 = ranked.iter().map(|r| r.estimated_points).sum();

    let issues: Vec<serde_json::Value> = ranked
        .iter()
        .map(|ri| {
            let issue = ri.issue;
            let verify = verify_hint(&issue.check_id, &result.url);
            let fix_summary = issue.manual_fix.clone().unwrap_or_else(|| {
                issue
                    .fix_prompt
                    .clone()
                    .unwrap_or_else(|| issue.description.clone())
            });

            serde_json::json!({
                "rank": ri.rank,
                "id": issue.check_id,
                "title": issue.title,
                "severity": format!("{:?}", issue.severity).to_lowercase(),
                "category": category_label(&issue.category),
                "points": ri.estimated_points,
                "applicability": ri.applicability.label(),
                "fix_summary": fix_summary,
                "verify": verify,
            })
        })
        .collect();

    let categories: Vec<serde_json::Value> = result
        .categories
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": category_display_name(&c.category),
                "score": c.score,
                "issues": c.issues_total,
            })
        })
        .collect();

    let stack = result
        .detected_stack
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let payload = serde_json::json!({
        "url": result.url,
        "score": result.overall_score,
        "scan_type": result.scan_type,
        "detected_stack": stack,
        "scanned_at": result.timestamp,
        "recoverable_points": total_recoverable,
        "issues": issues,
        "categories": categories,
    });

    serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("failed to serialize issues.json: {}", e))
}
