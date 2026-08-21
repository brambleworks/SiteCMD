use super::super::*;

#[test]
fn skips_sensitive_auth_for_inline_admin_key_guard() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/index.ts",
        r#"
                import { Hono } from "hono";

                type Bindings = {
                  ADMIN_KEY: string;
                };

                const app = new Hono<{ Bindings: Bindings }>();

                app.get("/api/admin/emails", async (c) => {
                  const key = c.req.query("key");
                  if (!c.env.ADMIN_KEY || !key || key !== c.env.ADMIN_KEY) {
                    return c.json({ error: "Unauthorized" }, 401);
                  }
                  return c.json({ ok: true });
                });

                export default app;
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("sensitive-auth:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("sensitive-authz:")));
}

#[test]
fn skips_sensitive_auth_when_next_middleware_covers_route() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "next": "^16.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "src/middleware.ts",
        r#"
                import { withAuth } from "next-auth/middleware";

                export default withAuth();

                export const config = {
                  matcher: ["/api/admin/:path*"],
                };
            "#,
    );
    write_file(
        temp.path(),
        "app/api/admin/users/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ email: body.email });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("sensitive-auth:")));
}

#[test]
fn detects_client_auth_without_server_enforcement() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "auth-app",
                  "dependencies": {
                    "next": "^16.0.0",
                    "next-auth": "^5.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/dashboard/page.tsx",
        r#"
                "use client";
                import { useSession } from "next-auth/react";

                export default function DashboardPage() {
                  const { data } = useSession();
                  return <main>{data?.user?.email}</main>;
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/account/route.ts",
        r#"
                export async function PATCH(request: Request) {
                  const body = await request.json();
                  return Response.json({ email: body.email });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue
                .id
                .starts_with("client-auth-without-server-enforcement:")
        })
        .expect("client-auth/server-enforcement review");
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("does not prove"));
}

#[test]
fn skips_client_auth_without_server_enforcement_when_middleware_exists() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "auth-app",
                  "dependencies": {
                    "next": "^16.0.0",
                    "next-auth": "^5.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/dashboard/page.tsx",
        r#"
                "use client";
                import { useSession } from "next-auth/react";

                export default function DashboardPage() {
                  const { data } = useSession();
                  return <main>{data?.user?.email}</main>;
                }
            "#,
    );
    write_file(
        temp.path(),
        "src/middleware.ts",
        r#"
                import { withAuth } from "next-auth/middleware";

                export default withAuth();
            "#,
    );
    write_file(
        temp.path(),
        "app/api/account/route.ts",
        r#"
                export async function PATCH(request: Request) {
                  const body = await request.json();
                  return Response.json({ email: body.email });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report.issues.iter().any(|issue| issue
        .id
        .starts_with("client-auth-without-server-enforcement:")));
}

#[test]
fn detects_database_access_scattered_across_routes() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "db-app",
                  "dependencies": {
                    "@prisma/client": "^6.0.0",
                    "next": "^16.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/orders/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";
                export async function POST() {
                  const prisma = new PrismaClient();
                  await prisma.order.create({ data: { status: "pending" } });
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/users/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";
                export async function GET() {
                  const prisma = new PrismaClient();
                  return Response.json(await prisma.user.findMany());
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/billing/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";
                export async function POST() {
                  const prisma = new PrismaClient();
                  await prisma.invoice.create({ data: { total: 100 } });
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("db-scattered-across-routes:"))
        .expect("route-local database access review");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("does not prove"));
}

#[test]
fn write_route_talking_to_the_database_directly_is_flagged_as_db_in_route() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/items/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";

                export async function POST(req: Request) {
                  const body = await req.json();
                  const prisma = new PrismaClient();
                  const created = await prisma.item.create({ data: { name: body.name } });
                  return Response.json(created);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("db-in-route:")),
        "expected db-in-route, got: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn client_component_touching_the_database_is_flagged_as_client_db_access() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Dashboard.tsx",
        r#"
                'use client';
                import { PrismaClient } from "@prisma/client";

                const prisma = new PrismaClient();

                export async function loadRows() {
                  return prisma.metric.findMany({ take: 20 });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("client-db-access:"))
        .expect("client-db-access issue"); // allow-expect: test assertion
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
}

#[test]
fn supabase_browser_data_api_is_not_server_database_leakage() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/PublishedPosts.tsx",
        r#"
                'use client';
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");
                export async function loadPublishedPosts() {
                  return supabase.from("posts").select("*").eq("published", true);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .all(|issue| !issue.id.starts_with("client-db-access:")));
}
