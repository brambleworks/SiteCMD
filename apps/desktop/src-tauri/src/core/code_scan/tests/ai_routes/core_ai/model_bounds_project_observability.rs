use super::super::super::*;

#[test]
fn skips_user_controlled_ai_model_and_settings_when_allowlisted_and_bounded() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import { z } from "zod";
                import { streamText } from "ai";
                import { openai } from "@ai-sdk/openai";

                const allowedModels = ["gpt-4.1-mini", "gpt-4.1"] as const;

                export async function POST(request: Request) {
                  const body = z.object({
                    model: z.enum(allowedModels),
                    message: z.string().min(1),
                    maxTokens: z.number().int().min(32).max(800),
                    temperature: z.number().min(0).max(1),
                  }).parse(await request.json());

                  const result = streamText({
                    model: openai(body.model),
                    messages: [{ role: "user", content: body.message }],
                    maxTokens: Math.min(body.maxTokens, 800),
                    temperature: Math.max(0, Math.min(body.temperature, 1)),
                  });

                  return result.toDataStreamResponse();
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-user-controlled-model:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-user-controlled-settings:")));
}

#[test]
fn detects_ai_heavy_project_without_observability_integration() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "openai": "^4.0.0", "ai": "^4.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import { streamText } from "ai";
                import { openai } from "@ai-sdk/openai";

                export async function POST(request: Request) {
                  const body = await request.json();
                  const result = streamText({
                    model: openai("gpt-4.1-mini"),
                    messages: [{ role: "user", content: body.message }],
                    maxTokens: 300,
                  });
                  return result.toDataStreamResponse();
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/summarize/route.ts",
        r#"
                import OpenAI from "openai";

                export async function POST(request: Request) {
                  const body = await request.json();
                  const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
                  return Response.json(await client.responses.create({ model: "gpt-4.1-mini", input: body.text }));
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report.issues.iter().any(|issue| issue
        .id
        .starts_with("ai-observability-integration-missing:")));
}

#[test]
fn skips_ai_heavy_project_integration_issue_when_ai_observability_exists() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "openai": "^4.0.0", "ai": "^4.0.0", "langfuse": "^3.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import { streamText } from "ai";
                import { openai } from "@ai-sdk/openai";

                export async function POST(request: Request) {
                  const body = await request.json();
                  const result = streamText({
                    model: openai("gpt-4.1-mini"),
                    messages: [{ role: "user", content: body.message }],
                    maxTokens: 300,
                  });
                  return result.toDataStreamResponse();
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/summarize/route.ts",
        r#"
                import OpenAI from "openai";

                export async function POST(request: Request) {
                  const body = await request.json();
                  const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
                  return Response.json(await client.responses.create({ model: "gpt-4.1-mini", input: body.text }));
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report.issues.iter().any(|issue| issue
        .id
        .starts_with("ai-observability-integration-missing:")));
}

#[test]
fn llm_route_without_output_cap_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import OpenAI from "openai";

                const client = new OpenAI({ maxRetries: 2 });

                export async function POST(req: Request) {
                  const body = await req.json();
                  const completion = await client.chat.completions.create({
                    model: "gpt-4.1-mini",
                    messages: body.messages,
                  });
                  return Response.json(completion.choices[0].message);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-output-cap:"))
        .unwrap_or_else(|| {
            panic!(
                "expected ai-output-cap, got: {:?}",
                report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
            )
        });
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.contains("No recognized"));
    assert!(issue.description.contains("does not prove"));
    assert!(issue.description.contains("wrapper"));
}

#[test]
fn llm_route_with_explicit_output_cap_is_not_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import OpenAI from "openai";

                const client = new OpenAI({ maxRetries: 2 });

                export async function POST(req: Request) {
                  const body = await req.json();
                  const completion = await client.chat.completions.create({
                    model: "gpt-4.1-mini",
                    max_tokens: 256,
                    messages: body.messages,
                  });
                  return Response.json(completion.choices[0].message);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("ai-output-cap:")),
        "explicit max_tokens should satisfy ai-output-cap, got: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}
