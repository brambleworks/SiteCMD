use crate::checks::CheckStatus;
use crate::core::scanner::ScanResult;

/// Populate `fix_prompt` on failing/warning issues that don't already have one.
/// This keeps CLI exports and MCP workspace fallback aligned with desktop
/// persistence, where prompts are normally generated.
pub(super) fn enrich_with_fix_prompts(mut result: ScanResult) -> ScanResult {
    let url = result.url.clone();
    let detected_stack = result.detected_stack.clone();
    for issue in result.issues.iter_mut() {
        if issue.fix_prompt.is_some() {
            continue;
        }
        if matches!(issue.status, CheckStatus::Fail | CheckStatus::Warn) {
            issue.fix_prompt = Some(crate::ai::build_fix_prompt(
                issue,
                &url,
                detected_stack.as_ref(),
            ));
        }
    }
    result
}
