use super::super::*;

#[test]
fn detects_cookie_backed_write_missing_csrf() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/account/route.ts",
        r#"
                import { cookies } from "next/headers";

                export async function POST(request: Request) {
                  const session = cookies().get("session-token");
                  const body = await request.formData();
                  if (!session) return new Response("nope", { status: 401 });
                  return Response.json({ email: body.get("email") });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("csrf-missing:")));
}

#[test]
fn skips_cookie_write_guarded_by_same_origin_helper_call() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/account/route.ts",
        r#"
                import { cookies, headers } from "next/headers";
                import { isSameOrigin } from "@/lib/admin-session";

                export async function POST(request: Request) {
                  const origin = headers().get("origin");
                  const host = headers().get("x-forwarded-host") ?? headers().get("host");
                  if (!isSameOrigin(origin, host)) {
                    return new Response("forbidden", { status: 403 });
                  }
                  const session = cookies().get("session-token");
                  const body = await request.formData();
                  if (!session) return new Response("nope", { status: 401 });
                  return Response.json({ email: body.get("email") });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("csrf-missing:")));
}

#[test]
fn skips_cookie_write_guarded_by_csrf_token_header() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/account/route.ts",
        r#"
                import { cookies } from "next/headers";

                export async function POST(request: Request) {
                  const session = cookies().get("session-token");
                  const csrfToken = request.headers.get("X-CSRF-Token");
                  if (!session || !csrfToken) return new Response("nope", { status: 401 });
                  const body = await request.formData();
                  return Response.json({ email: body.get("email") });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("csrf-missing:")));
}

#[test]
fn detects_unsafe_raw_sql_path() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/search/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";

                export async function POST(request: Request) {
                  const prisma = new PrismaClient();
                  const body = await request.json();
                  await prisma.$queryRawUnsafe(`SELECT * FROM users WHERE email = '${body.email}'`);
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("raw-sql-unsafe:")));
}

#[test]
fn detects_interpolated_sql_in_normal_query_call() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/search/route.ts",
        r#"
                import pg from "pg";

                const pool = new pg.Pool();

                export async function POST(request: Request) {
                  const body = await request.json();
                  await pool.query(`SELECT * FROM users WHERE email = '${body.email}'`);
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("raw-sql-unsafe:")));
}

#[test]
fn detects_formatted_sql_query_in_rust_handler() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/routes/admin.rs",
        r#"
                async fn create_user(pool: sqlx::PgPool, email: String) {
                    let _ = sqlx::query(&format!("SELECT * FROM users WHERE email = '{}'", email))
                        .execute(&pool)
                        .await;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("raw-sql-unsafe:")));
}

#[test]
fn skips_parameterized_pg_query() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/search/route.ts",
        r#"
                import pg from "pg";

                const pool = new pg.Pool();

                export async function POST(request: Request) {
                  const body = await request.json();
                  await pool.query("SELECT * FROM users WHERE email = $1", [body.email]);
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("raw-sql-unsafe:")));
}

#[test]
fn skips_csrf_for_next_server_actions_guarded_by_the_framework_origin_check() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "lib/actions/tips.ts",
        r#""use server";

import { cookies } from "next/headers";
import { createClient } from "@/lib/supabase/server";

export async function submitTip(formData: FormData) {
  const session = cookies().get("session-token");
  if (!session) return { success: false };
  const supabase = await createClient();
  await supabase.from("tips").insert({ text: formData.get("text") });
  return { success: true };
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("csrf-missing:")),
        "Next.js rejects cross-origin Server Action invocations itself, got {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}
