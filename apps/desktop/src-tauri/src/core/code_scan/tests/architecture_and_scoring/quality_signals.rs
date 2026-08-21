use super::super::*;
use crate::checks::IssueConfidence;

#[test]
fn god_module_is_flagged_for_large_multi_responsibility_service() {
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
                const prisma = new PrismaClient();

                export async function processOrder(email: string, priceId: string) {
                  const session = await getServerSession();
                  const input = z.object({ email: z.string(), priceId: z.string() }).parse({ email, priceId });
                  await prisma.order.create({ data: { email: input.email } });
                  await stripe.checkout.sessions.create({ line_items: [{ price: input.priceId, quantity: 1 }] });
                  await resend.emails.send({ to: input.email, subject: "hi", html: "<p>ok</p>" });
                  return Boolean(session);
                }
            "#,
    );
    for _ in 0..920 {
        content.push_str("// filler to simulate a giant service module\n");
    }
    write_file(temp.path(), "src/lib/billing.ts", &content);

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("god-module:"))
        .unwrap_or_else(|| {
            panic!(
                "expected god-module, got: {:?}",
                report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
            )
        });
    assert!(issue.title.contains("multiple detected responsibilities"));
    assert!(issue.description.contains("does not establish"));
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("oversized-module:")),
        "the more specific god-module finding must suppress oversized-module"
    );
}

#[test]
fn vcs_checkout_and_file_identity_do_not_invent_module_responsibilities() {
    let temp = TempDir::new().unwrap();
    let mut content = String::from(
        r#"
                enum OccurrenceLocation {
                    File { path: String },
                }

                async fn validate_record(id: i64) {
                    let _row = sqlx::query("SELECT id FROM scan_records WHERE id = ?")
                        .bind(id)
                        .fetch_optional(&pool)
                        .await;
                }

                // Preserve the source-control checkout captured during the file walk.
            "#,
    );
    for _ in 0..520 {
        content.push_str("// cohesive persistence implementation\n");
    }
    write_file(temp.path(), "src/db/persistence.rs", &content);

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("god-module:")),
        "File and checkout vocabulary must not become upload and billing responsibilities"
    );
}

#[test]
fn empty_catch_blocks_are_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/sync.ts",
        r#"
                import { push, pull } from "./transport";

                export async function syncAll(items: string[]) {
                  try { await push(items); } catch (e) {}
                  try { await pull(items); } catch (e) {}
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("empty-catch-blocks:"))
        .expect("empty-catch-blocks issue");
    assert_eq!(issue.confidence, IssueConfidence::Confirmed);
}

#[test]
fn console_log_only_error_handling_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/tasks.ts",
        r#"
                import { runA, runB, runC } from "./jobs";

                export async function runAll() {
                  try { await runA(); } catch (err) { console.log(err); }
                  try { await runB(); } catch (err) { console.warn(err); }
                  try { await runC(); } catch (err) { console.error(err); }
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("console-log-error-handling:")));
}

#[test]
fn ai_conversation_artifacts_in_comments_are_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/search.ts",
        r#"
                // Here's how the debounce works for the search input
                export function debounce(fn: () => void, ms: number) {
                  let handle: ReturnType<typeof setTimeout>;
                  // I've added error handling below as requested
                  return () => {
                    clearTimeout(handle);
                    handle = setTimeout(fn, ms);
                  };
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-conversation-artifacts:")));
}

#[test]
fn hardcoded_localhost_url_in_route_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/notify/route.ts",
        r#"
                export async function GET() {
                  const res = await fetch("http://localhost:3000/api/internal/status");
                  return Response.json(await res.json());
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("hardcoded-localhost-url:"))
        .expect("hardcoded-localhost-url issue"); // allow-expect: test assertion
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(issue.confidence, IssueConfidence::NeedsReview);
    assert!(issue.description.contains("intentional loopback sidecar"));
    assert!(!issue.description.contains("will break"));
    assert!(!issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("|| 'http://localhost"));
    assert!(issue
        .verify_hint
        .as_deref()
        .unwrap_or_default()
        .contains("deployment topology"));
}

#[test]
fn typescript_any_abuse_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/parse.ts",
        r#"
                export function normalize(input: any, config: any): any {
                  const state: any = {};
                  const cache: any = {};
                  const helpers: any = {};
                  const output: any = { input, config, state, cache, helpers };
                  return output;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("typescript-any-abuse:")));
}

#[test]
fn weak_default_credential_fallback_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/config.ts",
        r#"
                export const sessionSecret = process.env.SESSION_SECRET || "changeme";
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("weak-default-credential:")));
}

#[test]
fn localstorage_auth_token_is_flagged_on_frontend_surface() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/AuthProvider.tsx",
        r#"
                export function persistSession(token: string) {
                  localStorage.setItem("token", token);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("localstorage-auth-token:"))
        .expect("localstorage-auth-token issue"); // allow-expect: test assertion
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(issue.confidence, IssueConfidence::NeedsReview);
    assert!(issue.description.contains("does not prove"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("Choose the authentication architecture"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("CSRF"));
    assert!(!issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("XSS cannot steal"));
}

#[test]
fn unbounded_list_query_in_route_is_flagged_as_no_pagination() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/items/route.ts",
        r#"
                import { prisma } from "../../../lib/db";

                export async function GET() {
                  const items = await prisma.item.findMany();
                  return Response.json(items);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("no-pagination:"))
        .expect("no-pagination issue"); // allow-expect: test assertion
    assert_eq!(issue.severity, Severity::Medium);
    assert!(issue.title.contains("No recognized"));
    assert!(issue.description.contains("does not establish"));
}

#[test]
fn list_query_with_variable_limit_and_cursor_is_bounded() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/pages/api/emails.ts",
        r#"
                const MAX_LIMIT = 500;
                export const GET = async ({ request, env }) => {
                  const requested = Number(new URL(request.url).searchParams.get("limit"));
                  const limit = Math.min(Math.max(requested, 1), MAX_LIMIT);
                  const cursor = new URL(request.url).searchParams.get("cursor") ?? undefined;
                  const page = await env.EMAILS.list({ cursor, limit });
                  return Response.json(page);
                };
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("no-pagination:")),
        "an explicit limit and cursor must count as a bounded list query"
    );
}

#[test]
fn password_stored_without_hashing_is_flagged_as_plaintext_password() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                import { prisma } from "../../../lib/db";

                export async function POST(req: Request) {
                  const body = await req.json();
                  const user = await prisma.user.create({
                    data: { email: body.email, password: body.password },
                  });
                  return Response.json({ id: user.id });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("plaintext-password:"))
        .expect("plaintext-password issue"); // allow-expect: test assertion
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.to_ascii_lowercase().contains("possible"));
    assert!(issue.description.contains("does not inspect"));
    assert!(issue
        .likely_fix
        .as_deref()
        .is_some_and(|fix| fix.contains("slow password-hashing")
            && fix.contains("existing authentication hooks")));
}

#[test]
fn single_record_query_in_loop_is_flagged_as_n_plus_one() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/report.ts",
        r#"
                import { prisma } from "./db";

                export async function loadOwners(ids: string[]) {
                  const owners = [];
                  for (const id of ids) {
                    const owner = await prisma.user.findUnique({ where: { id } });
                    owners.push(owner);
                  }
                  return owners;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("n-plus-one-query:"))
        .expect("n-plus-one-query issue"); // allow-expect: test assertion
    assert!(issue.title.contains("Possible"));
    assert!(issue.description.contains("does not establish"));
}

#[test]
fn in_memory_lookups_inside_loops_are_not_flagged_as_n_plus_one() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/lib/calendar.ts",
        r#"
                export function groupEvents(events: Event[]) {
                  const byDate = new Map<string, Event[]>();
                  for (const event of events) {
                    const current = byDate.get(event.date) ?? [];
                    byDate.set(event.date, [...current, event]);
                  }
                  return events.map((event) => {
                    const cached = byDate.get(event.date);
                    return { event, cached };
                  });
                }
            "#,
    );
    write_file(
        temp.path(),
        "src/lib/cache.py",
        r#"
                def collect_cached(items, cache):
                    found = []
                    for item in items:
                        found.append(cache.get(item))
                    return found
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("n-plus-one-query:")),
        "in-memory lookup flagged: {:?}",
        report
            .issues
            .iter()
            .map(|issue| &issue.id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn hashed_password_write_is_not_flagged_as_plaintext_password() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                import bcrypt from "bcryptjs";
                import { prisma } from "../../../lib/db";

                export async function POST(req: Request) {
                  const body = await req.json();
                  const hashed = await bcrypt.hash(body.password, 12);
                  const user = await prisma.user.create({
                    data: { email: body.email, password: hashed },
                  });
                  return Response.json({ id: user.id });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("plaintext-password:")),
        "hashed write flagged: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn password_relation_write_is_not_flagged_as_plaintext_password() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                import { prisma } from "../../../lib/db";
                import { getPasswordHash } from "../../../lib/auth";

                export async function POST(req: Request) {
                  const body = await req.json();
                  const user = await prisma.user.create({
                    data: {
                      email: body.email,
                      password: { create: { hash: await getPasswordHash(body.password) } },
                    },
                  });
                  return Response.json({ id: user.id });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("plaintext-password:")),
        "relation write flagged: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}
