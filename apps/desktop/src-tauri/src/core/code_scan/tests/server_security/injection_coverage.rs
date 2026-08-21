use super::super::*;

fn issue_ids(report: &CodeScanReport) -> Vec<String> {
    report.issues.iter().map(|issue| issue.id.clone()).collect()
}

fn has_issue(report: &CodeScanReport, prefix: &str) -> bool {
    report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with(prefix))
}

#[test]
fn js_route_with_exec_and_body_parsing_is_flagged_as_shell_injection() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/deploy/route.ts",
        r#"import { exec } from "child_process";

export async function POST(req: Request) {
  const body = await req.json();
  const command = commandForTarget(body.target);
  exec(command);
  return Response.json({ ok: true });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "shell-injection:"),
        "expected shell-injection, got: {:?}",
        issue_ids(&report)
    );
    // No request accessor appears inside the exec call, so the precise
    // sink check must stay quiet and leave this case to shell-injection.
    assert!(
        !has_issue(&report, "js-command-injection:"),
        "js-command-injection should not claim the indirect case, got: {:?}",
        issue_ids(&report)
    );

    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("shell-injection:"))
        .expect("shell-injection issue");
    assert_eq!(issue.severity, crate::checks::Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.to_ascii_lowercase().contains("possible"));
    assert!(
        issue.description.contains("does not prove"),
        "the description must disclose the lack of data-flow proof: {}",
        issue.description
    );
    assert!(
        issue.evidence.as_deref().is_some_and(
            |evidence| evidence.contains("same file") && evidence.contains("not proven")
        ),
        "evidence must say exactly what the co-occurrence detector observed: {:?}",
        issue.evidence
    );
    assert!(
        issue
            .likely_fix
            .as_deref()
            .is_some_and(|fix| fix.contains("fixed executable") && fix.contains("argument array")),
        "fix should lead with a shell-free process API: {:?}",
        issue.likely_fix
    );
    assert!(
        !issue
            .likely_fix
            .as_deref()
            .unwrap_or_default()
            .contains("escaping library"),
        "shell escaping must not be presented as a general sanitizer"
    );
    assert!(
        issue
            .verify_hint
            .as_deref()
            .is_some_and(|hint| hint.contains("leading-option") && hint.contains("metacharacter")),
        "verification should cover both shell syntax and option injection: {:?}",
        issue.verify_hint
    );
}

#[test]
fn js_route_with_eval_and_body_parsing_is_flagged_as_eval_exec_injection() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/formula/route.ts",
        r#"export async function POST(req: Request) {
  const body = await req.json();
  const result = eval(body.formula);
  return Response.json({ result });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "eval-exec-injection:"),
        "expected eval-exec-injection, got: {:?}",
        issue_ids(&report)
    );
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("eval-exec-injection:"))
        .expect("eval-exec-injection issue");
    assert_eq!(issue.severity, crate::checks::Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.to_ascii_lowercase().contains("possible"));
    assert!(issue.description.contains("does not prove"));
    assert!(issue
        .evidence
        .as_deref()
        .is_some_and(|evidence| evidence.contains("same file") && evidence.contains("not proven")));
}

#[test]
fn constant_eval_next_to_unrelated_request_parsing_is_not_called_injection() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/compat/route.ts",
        r#"export async function POST(req: Request) {
  const body = await req.json();
  const legacyConstant = eval("1 + 1");
  return Response.json({ legacyConstant, received: Boolean(body) });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "eval-exec-injection:"),
        "a literal-only eval is risky code quality, but request injection was not observed: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn regex_exec_without_child_process_is_not_shell_injection() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/validate/route.ts",
        r#"const SLUG_PATTERN = /^[a-z0-9-]+$/;

export async function POST(req: Request) {
  const body = await req.json();
  // exec( appears here only as RegExp.prototype.exec and in this comment.
  const match = SLUG_PATTERN.exec(body.slug);
  return Response.json({ valid: match !== null });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "shell-injection:"),
        "RegExp.prototype.exec misread as shell execution, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn shell_escaped_exec_is_not_flagged_as_shell_injection() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/deploy/route.ts",
        r#"import { exec } from "child_process";
import shellEscape from "shell-escape";

export async function POST(req: Request) {
  const body = await req.json();
  const command = "deploy --target " + shellEscape([body.target]);
  exec(command);
  return Response.json({ ok: true });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "shell-injection:"),
        "shell-escaped exec should not be flagged, got: {:?}",
        issue_ids(&report)
    );
}
