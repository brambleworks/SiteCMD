//! Cancellation must stop the audit inside the analyze pass, not after it.

use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

/// Analyze workers stride the file list, so the number of files has to stay
/// well above the number of workers for "stopped early" to mean anything.
fn analyze_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
}

fn project_with_source_files(count: usize) -> TempDir {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{"name":"cancel-fixture","version":"1.0.0"}"#,
    );
    for index in 0..count {
        write_file(
            temp.path(),
            &format!("src/module{index}.ts"),
            "export const value = 1;\n",
        );
    }
    temp
}

#[test]
fn a_pre_cancelled_audit_produces_no_report_and_starts_no_stage() {
    let temp = project_with_source_files(8);
    let stages: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

    let result = audit_project_with_control(
        temp.path(),
        CodeScanOptions::default(),
        |progress| stages.lock().unwrap().push(progress.check_id),
        || true,
    );

    assert!(
        matches!(result, Err(CodeScanError::Cancelled)),
        "a pre-cancelled audit must report cancellation, not a report or a failure"
    );
    assert!(
        stages.lock().unwrap().is_empty(),
        "a pre-cancelled audit must not begin a stage: {:?}",
        stages.lock().unwrap()
    );
}

#[test]
fn cancelling_mid_pass_stops_before_every_file_is_analyzed() {
    // The probe only flips once the analyze pass has polled a handful of
    // files. A stage-only cancellation check polls barely half a dozen times
    // for the whole audit, so it would never reach this threshold and the
    // audit would return a finished report instead.
    const POLLS_BEFORE_CANCEL: usize = 12;
    let workers = analyze_worker_count();
    let file_count = (workers * 4).max(200);
    let temp = project_with_source_files(file_count);
    let analyzing = AtomicBool::new(false);
    let polls = AtomicUsize::new(0);
    let stages: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

    let result = audit_project_with_control(
        temp.path(),
        CodeScanOptions::default(),
        |progress| {
            if progress.check_id == "code-scan.analyze-source" {
                analyzing.store(true, Ordering::SeqCst);
            }
            stages.lock().unwrap().push(progress.check_id);
        },
        || {
            analyzing.load(Ordering::SeqCst)
                && polls.fetch_add(1, Ordering::SeqCst) >= POLLS_BEFORE_CANCEL
        },
    );

    assert!(
        matches!(result, Err(CodeScanError::Cancelled)),
        "cancellation during the analyze pass must abandon the audit without a report"
    );
    let observed = polls.load(Ordering::SeqCst);
    assert!(
        observed < file_count,
        "the analyze pass must stop early; it polled {observed} times over {file_count} files"
    );
    let stages = stages.lock().unwrap();
    assert!(stages.contains(&"code-scan.analyze-source"));
    assert!(
        !stages.contains(&"code-scan.finalize"),
        "a cancelled audit must not reach the finalize stage: {stages:?}"
    );
    assert!(
        !stages.contains(&"code-scan.supply-chain"),
        "a cancelled audit must not start the stage after analyze: {stages:?}"
    );
}

#[test]
fn an_uncancelled_audit_still_produces_its_report() {
    let temp = project_with_source_files(8);

    let report =
        audit_project_with_control(temp.path(), CodeScanOptions::default(), |_| {}, || false)
            .expect("an uncancelled audit must still produce a report");

    assert!(!report.checked_at.is_empty());
}
