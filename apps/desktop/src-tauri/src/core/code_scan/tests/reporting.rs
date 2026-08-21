use super::*;

#[test]
fn review_format_includes_fix_and_verify_guidance() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import OpenAI from "openai";

                export async function POST(request: Request) {
                  const body = await request.json();
                  const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
                  return Response.json(await client.responses.create({ model: "gpt-4.1", input: body.message }));
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let rendered = format_report(&report, temp.path(), CodeScanReportFormat::Review).unwrap();
    assert!(rendered.contains("# SiteCMD Code Scan Review"));
    assert!(rendered.contains("- Best first fix:"));
    assert!(rendered.contains("- Verify:"));
    assert!(rendered.contains("- Source excerpt:"));
}

#[test]
fn github_format_emits_annotations() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/webhook/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.text();
                  return new Response(body);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let rendered = format_report(&report, temp.path(), CodeScanReportFormat::Github).unwrap();
    assert!(
        rendered.contains("::error file=")
            || rendered.contains("::warning file=")
            || rendered.contains("::notice file=")
    );
    assert!(rendered.contains("SiteCMD"));
    assert!(rendered.contains("Best first fix:"));
}

#[test]
fn summary_format_keeps_code_scan_labels_for_ai_safety_findings() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import OpenAI from "openai";

                export async function POST(request: Request) {
                  const body = await request.json();
                  const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
                  return Response.json(await client.responses.create({ model: "gpt-4.1", input: body.message }));
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let rendered = format_report(&report, temp.path(), CodeScanReportFormat::Summary).unwrap();
    assert!(rendered.contains("SiteCMD Code Scan"));
    assert!(rendered.contains("[AI Safety · ai-safety]"));
    assert!(!rendered.contains("SiteCMD AI Safety Scan"));
}
