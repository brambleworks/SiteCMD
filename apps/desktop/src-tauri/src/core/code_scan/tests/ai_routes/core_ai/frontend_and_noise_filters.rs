use super::super::super::*;
use crate::checks::IssueConfidence;

#[test]
fn detects_client_side_ai_sdk_usage() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/components/chat-widget.tsx",
        r#"
                'use client';

                import OpenAI from "openai";

                export function ChatWidget() {
                  const client = new OpenAI({ apiKey: process.env.NEXT_PUBLIC_OPENAI_API_KEY });
                  return <button onClick={() => client.responses.create({ model: "gpt-4.1-mini", input: "hi" })}>Chat</button>;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("client-ai-sdk:"))
        .expect("direct browser AI call finding");
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    let verify_hint = issue
        .verify_hint
        .as_deref()
        .expect("client AI finding should include a verification path");
    assert!(verify_hint.contains("long-lived credential"));
    assert!(verify_hint.contains("ephemeral"));
}

#[test]
fn client_side_ai_type_import_without_a_provider_call_is_not_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/components/chat-types.tsx",
        r#"
                'use client';
                import type OpenAI from "openai";

                export function ChatTypes({ value }: { value: OpenAI.Response }) {
                  return <pre>{JSON.stringify(value)}</pre>;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .all(|issue| !issue.id.starts_with("client-ai-sdk:")));
}

#[test]
fn detects_client_prefixed_secret_when_env_is_actually_read() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/components/Admin.tsx",
        r#"
                "use client";

                const serviceRoleKey = process.env.NEXT_PUBLIC_SUPABASE_SERVICE_ROLE_KEY;

                export function AdminPanel() {
                  return <pre>{serviceRoleKey ? "configured" : "missing"}</pre>;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("client-env-secret:"))
        .expect("client-env-secret issue");
    assert_eq!(issue.severity, crate::checks::Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.to_ascii_lowercase().contains("possible"));
    assert!(issue.description.contains("does not prove"));
    assert!(issue
        .likely_fix
        .as_deref()
        .is_some_and(|fix| fix.contains("confirm") && fix.contains("rotate")));
}

#[test]
fn skips_client_env_secret_for_server_only_error_message_mentions() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "lib/supabase/admin.ts",
        r#"
                import "server-only";
                import { createClient } from "@supabase/supabase-js";

                const supabaseUrl = process.env.NEXT_PUBLIC_SUPABASE_URL;
                const serviceRoleKey = process.env.SUPABASE_SERVICE_ROLE_KEY;

                if (!supabaseUrl || !serviceRoleKey) {
                  throw new Error("Missing NEXT_PUBLIC_SUPABASE_URL_OR_SUPABASE_SERVICE_ROLE_KEY");
                }

                export const supabaseAdmin = createClient(supabaseUrl, serviceRoleKey);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("client-env-secret:")));
}

#[test]
fn detects_excessive_jsx_inline_styles() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/components/Hero.tsx",
        r##"
                export function Hero() {
                  return (
                    <section style={{ padding: 32, background: "#111827" }}>
                      <h1 style={{ color: "#fff", fontSize: 42 }}>Ship faster</h1>
                      <p style={{ color: "#d1d5db", maxWidth: 560 }}>Make launch fixes obvious.</p>
                      <div style={{ display: "flex", gap: 12 }}>
                        <a style={{ color: "#111827", background: "#fff", padding: "12px 18px" }}>Start</a>
                      </div>
                    </section>
                  );
                }
            "##,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("jsx-inline-style-density:"))
        .expect("expected jsx-inline-style-density finding");
    assert_eq!(issue.severity, Severity::Low);
    assert_eq!(issue.confidence, IssueConfidence::Confirmed);
    assert!(issue.title.contains("Review repeated JSX inline styles"));
    assert!(issue.description.contains("not inherently incorrect"));
}

#[test]
fn skips_jsx_inline_style_finding_for_one_off_dynamic_style() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/components/StatusPill.tsx",
        r#"
                export function StatusPill({ accent }: { accent: string }) {
                  return (
                    <span className="pill" style={{ borderColor: accent }}>
                      Healthy
                    </span>
                  );
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("jsx-inline-style-density:")));
}

#[test]
fn skips_jsx_inline_style_finding_for_react_pdf_documents() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/reports/Report.tsx",
        r##"
                import { Page, Text, View } from "@/lib/react-pdf-browser";

                export function Report() {
                  return (
                    <Page style={{ padding: 32 }}>
                      <View style={{ marginBottom: 12 }}>
                        <Text style={{ fontSize: 18 }}>Summary</Text>
                        <Text style={{ color: "#444" }}>Healthy</Text>
                      </View>
                    </Page>
                  );
                }
            "##,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("jsx-inline-style-density:")));
}

#[test]
fn skips_unsafe_html_when_local_escape_helper_is_used() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/pages/results.ts",
        r#"
                function escapeHtml(str) {
                  return String(str)
                    .replace(/&/g, "&amp;")
                    .replace(/</g, "&lt;")
                    .replace(/>/g, "&gt;");
                }

                export function render(data) {
                  const html = '<div>' + escapeHtml(data.message) + '</div>';
                  document.getElementById("results").innerHTML = html;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("unsafe-html:")));
}

#[test]
fn skips_findings_inside_rust_test_modules() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib.rs",
        r###"
                #[tracing::instrument]
                pub fn ok() -> bool { true }

                #[cfg(test)]
                mod tests {
                    #[test]
                    fn fixture_contains_scary_strings() {
                        let sample = r##"
                            "use client";
                            import OpenAI from "openai";
                            const html = "<div>" + body + "</div>";
                            results.innerHTML = html;
                        "##;
                        assert!(!sample.is_empty());
                    }
                }
            "###,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("client-ai-sdk:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("unsafe-html:")));
}

#[test]
fn skips_runtime_guardrails_in_regex_pattern_registry_files() {
    let temp = TempDir::new().unwrap();
    let mut registry = String::from(
        r#"
                use std::sync::LazyLock;

                // Sink catalog; entries match e.g. prisma.user writes.
                static DANGEROUS_HTML_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
                    vec![
                        regex::Regex::new(r"innerHTML\s*=").unwrap(),
                        regex::Regex::new(r"router.post\(").unwrap(),
                        regex::Regex::new(r"openai").unwrap(),
                        regex::Regex::new(r"access-control-allow-origin").unwrap(),
                        regex::Regex::new(r"access-control-allow-credentials").unwrap(),
                        regex::Regex::new(r"db.update").unwrap(),
                    ]
                });
            "#,
    );
    for i in 0..920 {
        registry.push_str(&format!("// pattern catalog note {}\n", i));
    }
    write_file(temp.path(), "src/patterns.rs", &registry);

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("cors-credentials-wildcard:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-rate-limit:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("multi-write-no-transaction:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("oversized-module:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("god-module:")));
}

#[test]
fn oversized_high_risk_module_is_flagged() {
    let temp = TempDir::new().unwrap();
    let mut module = String::from(
        "import { prisma } from \"./db\";\n\nexport async function loadUser(id: string) {\n  return prisma.user.findUnique({ where: { id } });\n}\n",
    );
    for i in 0..920 {
        module.push_str(&format!(
            "export function helper{}(): number {{ return {}; }}\n",
            i, i
        ));
    }
    write_file(temp.path(), "src/lib/service.ts", &module);

    let report = audit_project(temp.path()).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("oversized-module:")),
        "expected oversized-module, got: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn skips_llm_checks_for_reference_mentions_without_runtime_usage() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/reference.ts",
        r#"
                export const AI_HINTS = {
                  openai: "Check env files and middleware for leaked keys",
                  anthropic: "Audit server-only routes before shipping",
                  gemini: "Keep provider keys off the client",
                };
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-retry-bounds:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-kill-switch-missing:")));
}

#[test]
fn llm_usage_without_retry_policy_is_flagged_as_ai_retry_bounds() {
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
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-retry-bounds:"))
        .unwrap_or_else(|| {
            panic!(
                "expected ai-retry-bounds, got: {:?}",
                report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
            )
        });
    assert_eq!(issue.severity, Severity::Low);
    assert!(issue.title.contains("No explicit"));
    assert!(issue.description.contains("zero retries"));
    assert!(issue.description.contains("does not prove"));
}

#[test]
fn llm_usage_with_explicit_retry_cap_is_not_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/ai.ts",
        r#"
                import OpenAI from "openai";

                const client = new OpenAI({ maxRetries: 2 });

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
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("ai-retry-bounds:")),
        "explicit maxRetries should satisfy ai-retry-bounds, got: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn skips_jsx_inline_style_finding_for_next_og_image_routes() {
    let temp = TempDir::new().unwrap();
    let og_image = r##"
                import { ImageResponse } from "next/og";

                export const size = { width: 1200, height: 630 };

                export default function OGImage() {
                  return new ImageResponse(
                    (
                      <div style={{ display: "flex", width: "100%", height: "100%", background: "#0a1628" }}>
                        <span style={{ fontSize: "28px", color: "#f0a500" }}>Visit</span>
                        <span style={{ fontSize: "72px", color: "#ffffff" }}>Your Team</span>
                        <span style={{ fontSize: "24px", color: "#94a3b8" }}>Guides</span>
                      </div>
                    ),
                    { ...size }
                  );
                }
            "##;
    write_file(temp.path(), "app/opengraph-image.tsx", og_image);
    write_file(temp.path(), "lib/league-pages/team-og.tsx", og_image);

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("jsx-inline-style-density:")),
        "Satori renders inline styles only, got {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn skips_jsx_inline_style_finding_when_every_style_is_runtime_derived() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "components/tools/CompareClient.tsx",
        r#"
                export function CompareClient({ left, right }: Props) {
                  return (
                    <div>
                      <div style={{ borderColor: left.primaryColor }} />
                      <span style={{ background: left.primaryColor }} />
                      <span style={{ background: right.primaryColor }} />
                      <p style={{ color: right.primaryColor }}>{right.name}</p>
                      <strong style={{ color: left.total <= right.total ? left.primaryColor : undefined }}>
                        {left.name}
                      </strong>
                    </div>
                  );
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("jsx-inline-style-density:")),
        "runtime-derived values belong inline by the rule's own contract, got {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn skips_unsafe_html_for_json_ld_scripts_serialized_with_json_stringify() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "lib/seo.ts",
        r#"
                import React from "react";

                export function JsonLd({ data }: { data: Record<string, unknown> }) {
                  return React.createElement("script", {
                    type: "application/ld+json",
                    dangerouslySetInnerHTML: {
                      __html: JSON.stringify(data).replace(/</g, "\\u003c"),
                    },
                  });
                }
            "#,
    );
    write_file(
        temp.path(),
        "components/Breadcrumbs.tsx",
        r#"
                export function Breadcrumbs({ json }: { json: object }) {
                  return <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(json) }} />;
                }
            "#,
    );
    // Negative control: JSON-LD beside the same serialization in a plain
    // element. JSON.stringify does not escape `<`, so that div is a real sink.
    write_file(
        temp.path(),
        "components/ProfileCard.tsx",
        r#"
                export function ProfileCard({ schema, userProfile }: Props) {
                  return (
                    <>
                      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(schema) }} />
                      <div dangerouslySetInnerHTML={{ __html: JSON.stringify(userProfile) }} />
                    </>
                  );
                }
            "#,
    );
    // Negative control: a JSON-LD file that also renders a raw HTML string.
    write_file(
        temp.path(),
        "components/RichText.tsx",
        r#"
                export function RichText({ html, json }: Props) {
                  return (
                    <>
                      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(json) }} />
                      <div dangerouslySetInnerHTML={{ __html: html }} />
                    </>
                  );
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = report.issues.iter().map(|i| &i.id).collect::<Vec<_>>();
    assert!(
        !ids.iter().any(|id| id.as_str() == "unsafe-html:lib/seo.ts"),
        "serialized JSON in a non-executing script type is not an HTML sink, got {:?}",
        ids
    );
    assert!(
        !ids.iter()
            .any(|id| id.as_str() == "unsafe-html:components/Breadcrumbs.tsx"),
        "the JSX form of the JSON-LD pattern is the same contract, got {:?}",
        ids
    );
    assert!(
        ids.iter()
            .any(|id| id.as_str() == "unsafe-html:components/RichText.tsx"),
        "negative control: a raw HTML sink beside JSON-LD keeps the finding, got {:?}",
        ids
    );
    assert!(
        ids.iter()
            .any(|id| id.as_str() == "unsafe-html:components/ProfileCard.tsx"),
        "negative control: serialized JSON in a plain element is still a markup sink, got {:?}",
        ids
    );
}
