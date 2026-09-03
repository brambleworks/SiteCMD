use super::super::*;

#[test]
fn column_zero_matches_point_at_their_own_line() {
    let temp = TempDir::new().unwrap();
    // Deliberate fake Stripe-shaped key, not a real credential.
    let content = "const keys = [\n\"sk_live_abcdefghijklmnopqrstu\", // gitleaks:allow\n];\nexport default keys;\n";
    write_file(temp.path(), "src/keys.js", content);
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("hardcoded-secret:"))
        .expect("hardcoded-secret issue"); // allow-expect: test assertion
    assert_eq!(
        issue.line,
        Some(2),
        "a column-0 match on line 2 must report line 2"
    );
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("does not verify"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("revoke or rotate"));
    assert!(issue
        .verify_hint
        .as_deref()
        .unwrap_or_default()
        .contains("old credential fails"));
}

#[test]
fn detects_god_route_with_many_responsibilities() {
    let temp = TempDir::new().unwrap();
    let mut content = String::from(
        r#"
                import { getServerSession } from "next-auth";
                import { z } from "zod";
                import { PrismaClient } from "@prisma/client";
                import Stripe from "stripe";
                import { Resend } from "resend";

                const stripe = new Stripe(process.env.STRIPE_SECRET!, { apiVersion: "2024-06-20" });
                const resend = new Resend(process.env.RESEND_API_KEY!);

                export async function POST(request: Request) {
                  const session = await getServerSession();
                  const body = z.object({ email: z.string(), priceId: z.string() }).parse(await request.json());
                  const prisma = new PrismaClient();
                  await prisma.order.create({ data: { email: body.email } });
                  await stripe.checkout.sessions.create({ line_items: [{ price: body.priceId, quantity: 1 }] });
                  await resend.emails.send({ to: body.email, subject: "hi", html: "<p>ok</p>" });
                  return Response.json({ ok: !!session });
                }
            "#,
    );
    for _ in 0..260 {
        content.push_str("// filler to simulate a giant vibe-coded route\n");
    }
    write_file(temp.path(), "app/api/billing/route.ts", &content);

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("god-route:"))
        .expect("god-route issue"); // allow-expect: test assertion
    assert_eq!(issue.confidence, crate::checks::IssueConfidence::High);
    assert!(issue.title.contains("multiple detected responsibilities"));
    assert!(issue.description.contains("does not establish"));
}

#[test]
fn detects_unexpected_registry_host_and_direct_url_dependency() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "registry-app",
                  "dependencies": {
                    "left-pad": "https://evil.example/left-pad.tgz",
                    "axios": "^1.7.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"
                {
                  "name": "registry-app",
                  "lockfileVersion": 3,
                  "packages": {
                    "": {
                      "dependencies": {
                        "left-pad": "1.3.0",
                        "axios": "1.7.2"
                      }
                    },
                    "node_modules/left-pad": {
                      "version": "1.3.0",
                      "resolved": "https://evil.example/left-pad-1.3.0.tgz"
                    },
                    "node_modules/axios": {
                      "version": "1.7.2",
                      "resolved": "https://npm.mirror.example/axios/-/axios-1.7.2.tgz"
                    }
                  }
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("direct-url-dependency:")));
    // A dependency declared as a direct URL resolves from that URL by
    // definition, so only the direct-url review applies to it.
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id == "registry-host-mismatch:package.json:left-pad"),
        "a URL spec names its own source, got {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
    let registry_issue = report
        .issues
        .iter()
        .find(|issue| issue.id == "registry-host-mismatch:package.json:axios")
        .expect("registry-host-mismatch issue"); // allow-expect: test assertion
    assert_eq!(registry_issue.severity, Severity::High);
    assert_eq!(
        registry_issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(registry_issue.description.contains("does not prove"));
}

#[test]
fn keeps_ai_safety_issues_inside_code_scan_report() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/ai.ts",
        r#"
                import OpenAI from "openai";

                const client = new OpenAI();

                export async function summarize(text: string) {
                  const completion = await client.chat.completions.create({
                    model: "gpt-4.1-mini",
                    messages: [{ role: "user", content: text }],
                  });
                  return completion.choices[0].message.content;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ai_issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-retry-bounds:"))
        .expect("expected ai-retry-bounds in the report"); // allow-expect: test assertion
    assert_eq!(ai_issue.category, "ai-safety");
    assert_eq!(code_issue_domain(ai_issue), CodeScanDomain::AiSafety);

    // The derived counts must describe the issues vec that carries them.
    assert_eq!(report.issue_count, report.issues.len());
    let by_severity = |severity: Severity| {
        report
            .issues
            .iter()
            .filter(|issue| issue.severity == severity)
            .count()
    };
    assert_eq!(report.critical_count, by_severity(Severity::Critical));
    assert_eq!(report.high_count, by_severity(Severity::High));
    assert_eq!(report.medium_count, by_severity(Severity::Medium));
    assert_eq!(report.low_count, by_severity(Severity::Low));
}

#[test]
fn classifies_database_domain_for_data_issue() {
    let issue_with_slug = |id: &str| CodeIssue {
        check_id: String::new(),
        id: id.into(),
        category: "data".into(),
        severity: Severity::High,
        title: "Local DB drift".into(),
        description: "Schema drift.".into(),
        relative_path: "db/schema.prisma".into(),
        absolute_path: "/tmp/db/schema.prisma".into(),
        line: Some(1),
        source_excerpt: None,
        evidence: None,
        why_now: None,
        likely_fix: None,
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        verify_hint: None,
    };

    let registered = issue_with_slug("local-sqlite-schema-drift:db/schema.prisma");
    assert_eq!(code_issue_domain(&registered), CodeScanDomain::Database);

    // An unregistered slug falls back to the category heuristic - "data"
    // still lands in the Database domain.
    let unregistered = issue_with_slug("not-a-registered-check:db/schema.prisma");
    assert_eq!(code_issue_domain(&unregistered), CodeScanDomain::Database);
}

#[test]
fn serializes_json_report_with_domain() {
    let report = CodeScanReport {
        skipped_scopes: Default::default(),
        checked_at: "2026-04-09T12:00:00Z".into(),
        framework: Some("Next.js".into()),
        issue_count: 1,
        critical_count: 0,
        high_count: 1,
        medium_count: 0,
        low_count: 0,
        issues: vec![CodeIssue {
            check_id: String::new(),
            id: "ai-timeout:file".into(),
            category: "ai-safety".into(),
            severity: Severity::High,
            title: "AI timeout".into(),
            description: "Missing timeout".into(),
            relative_path: "app/api/chat/route.ts".into(),
            absolute_path: "/tmp/app/api/chat/route.ts".into(),
            line: Some(1),
            source_excerpt: None,
            evidence: None,
            why_now: None,
            likely_fix: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        }],
    };

    let json =
        format_report(&report, Path::new("."), CodeScanReportFormat::Json).expect("json report");
    assert!(json.contains("\"domain\": \"ai-safety\""));
}
