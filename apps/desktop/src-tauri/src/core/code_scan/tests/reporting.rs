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
fn sarif_format_emits_a_rule_and_result_per_finding() {
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
    let rendered = format_report(&report, temp.path(), CodeScanReportFormat::Sarif).unwrap();
    let sarif: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(sarif["version"], "2.1.0");
    let rules = sarif["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules array");
    assert!(!rules.is_empty());
    let results = sarif["runs"][0]["results"]
        .as_array()
        .expect("results array");
    assert!(!results.is_empty());
    assert!(results[0]["ruleId"].as_str().is_some());
    assert!(
        results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            .as_str()
            .is_some()
    );
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
