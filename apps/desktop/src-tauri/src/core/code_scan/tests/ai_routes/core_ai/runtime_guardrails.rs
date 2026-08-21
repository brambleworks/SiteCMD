use super::super::super::*;
use crate::checks::IssueConfidence;

#[test]
fn detects_ai_loop_without_hard_stop() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "scripts/reconcile-ai.ts",
        r#"
                import OpenAI from "openai";

                async function runForever() {
                  const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
                  while (true) {
                    await client.responses.create({ model: "gpt-4.1-mini", input: "retry the backlog" });
                  }
                }

                runForever();
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-loop-risk:"))
        .expect("expected ai-loop-risk finding");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(issue.confidence, IssueConfidence::NeedsReview);
    assert!(issue.title.contains("Possible unbounded AI loop"));
    assert!(issue.description.contains("same scanned file"));
    assert!(issue.description.contains("does not prove"));
}

#[test]
fn does_not_treat_a_cron_schedule_as_an_unbounded_ai_loop() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "scripts/daily-summary.ts",
        r#"
                import cron from "node-cron";
                import OpenAI from "openai";

                const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
                cron.schedule("0 8 * * *", async () => {
                  await client.responses.create({
                    model: "gpt-4.1-mini",
                    input: "summarize yesterday",
                    max_output_tokens: 200,
                  });
                });
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-loop-risk:")));
}

#[test]
fn skips_guarded_route() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import { z } from "zod";
                import { Ratelimit } from "@upstash/ratelimit";
                import pLimit from "p-limit";
                import OpenAI from "openai";

                export async function POST(request: Request) {
                  const controller = new AbortController();
                  const body = await request.json();
                  const schema = z.object({ message: z.string().min(1) });
                  const input = schema.parse(body);
                  const cacheKey = `chat:${input.message}`;
                  const cached = await redis.get(cacheKey);
                  if (cached) {
                    return Response.json(cached);
                  }
                  const limit = pLimit(2);
                  const requestId = crypto.randomUUID();
                  const monthlyBudget = 5000;
                  const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY, maxRetries: 2 });
                  const completion = await limit(() => client.responses.create({
                    model: "gpt-4.1",
                    input: input.message,
                    max_output_tokens: 300,
                    signal: controller.signal,
                  }));
                  await redis.set(cacheKey, completion, { ex: 60 });
                  console.info({ requestId, monthlyBudget, total_tokens: completion.usage?.total_tokens });
                  return Response.json(completion);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-rate-limit:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-timeout:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-concurrency:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-spend-guardrails:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-observability:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-cache-dedupe:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("request-validation:")));
}

#[test]
fn detects_ai_sdk_route_without_observability_or_dedupe() {
    let temp = TempDir::new().unwrap();
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

    let report = audit_project(temp.path()).unwrap();
    let observability = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-observability:"))
        .expect("expected ai-observability finding");
    assert_eq!(observability.severity, Severity::Medium);
    assert_eq!(observability.confidence, IssueConfidence::NeedsReview);
    assert!(observability.title.starts_with("No recognized"));
    assert!(observability.description.contains("scanned file"));
    assert!(observability.description.contains("prompts or responses"));

    let dedupe = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-cache-dedupe:"))
        .expect("expected ai-cache-dedupe finding");
    assert_eq!(dedupe.severity, Severity::Low);
    assert_eq!(dedupe.confidence, IssueConfidence::NeedsReview);
    assert!(dedupe.title.contains("duplicate-request handling"));
    assert!(dedupe.description.contains("not universally appropriate"));
    assert!(dedupe
        .likely_fix
        .as_deref()
        .is_some_and(|text| text.contains("only when repeated requests should share work")));
}

#[test]
fn detects_user_controlled_ai_model_and_settings_without_bounds() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import { streamText } from "ai";
                import { openai } from "@ai-sdk/openai";

                export async function POST(request: Request) {
                  const body = await request.json();
                  const result = streamText({
                    model: openai(body.model),
                    messages: [{ role: "user", content: body.message }],
                    maxTokens: body.maxTokens,
                    temperature: body.temperature,
                  });

                  return result.toDataStreamResponse();
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let model = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-user-controlled-model:"))
        .expect("expected ai-user-controlled-model finding");
    assert_eq!(model.severity, Severity::Medium);
    assert_eq!(model.confidence, IssueConfidence::NeedsReview);
    assert!(model.title.contains("Request-derived model selector"));
    assert!(model.description.contains("scanned file"));

    let settings = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-user-controlled-settings:"))
        .expect("expected ai-user-controlled-settings finding");
    assert_eq!(settings.severity, Severity::Medium);
    assert!(settings.title.contains("output limit"));
}

#[test]
fn request_controlled_sampling_settings_are_medium_not_high() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import { streamText } from "ai";
                import { openai } from "@ai-sdk/openai";

                export async function POST(request: Request) {
                  const body = await request.json();
                  return streamText({
                    model: openai("gpt-4.1-mini"),
                    prompt: body.message,
                    temperature: body.temperature,
                    maxTokens: 300,
                  }).toDataStreamResponse();
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-user-controlled-settings:"))
        .expect("expected ai-user-controlled-settings finding");
    assert_eq!(issue.severity, Severity::Medium);
    assert!(issue.title.contains("generation setting"));
}

#[test]
fn skips_ai_sdk_observability_and_dedupe_when_hooks_exist() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import { streamText } from "ai";
                import { openai } from "@ai-sdk/openai";
                import pLimit from "p-limit";
                import { Ratelimit } from "@upstash/ratelimit";

                export async function POST(request: Request) {
                  const controller = new AbortController();
                  const body = await request.json();
                  const key = `chat:${body.threadId}:${body.message}`;
                  const cached = await redis.get(key);
                  if (cached) {
                    return Response.json(cached);
                  }

                  const limit = pLimit(2);
                  const requestId = crypto.randomUUID();
                  const monthlyBudget = 100;
                  const result = await limit(() =>
                    streamText({
                      model: openai("gpt-4.1-mini"),
                      messages: [{ role: "user", content: body.message }],
                      maxTokens: 300,
                      abortSignal: controller.signal,
                      maxRetries: 2,
                      onFinish({ usage, providerMetadata }) {
                        console.info({
                          requestId,
                          monthlyBudget,
                          totalTokens: usage?.totalTokens,
                          providerMetadata,
                        });
                      },
                    }),
                  );

                  await redis.set(key, { ok: true }, { ex: 60 });
                  return result.toDataStreamResponse();
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-observability:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-cache-dedupe:")));
}
