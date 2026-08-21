use super::super::super::*;

#[test]
fn detects_multi_tenant_route_missing_workspace_scope() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/workspaces/projects/route.ts",
        r#"
                import { auth } from "@/auth";
                import { PrismaClient } from "@prisma/client";

                const prisma = new PrismaClient();

                export async function GET() {
                  const session = await auth();
                  if (!session?.user) {
                    return new Response("forbidden", { status: 403 });
                  }

                  return Response.json(
                    await prisma.project.findMany({
                      where: { archived: false },
                    }),
                  );
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("tenant-scope-missing:")));
}

#[test]
fn skips_multi_tenant_scope_issue_when_workspace_filter_is_present() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/workspaces/projects/route.ts",
        r#"
                import { auth } from "@/auth";
                import { PrismaClient } from "@prisma/client";

                const prisma = new PrismaClient();

                export async function GET() {
                  const session = await auth();
                  if (!session?.user?.workspaceId) {
                    return new Response("forbidden", { status: 403 });
                  }

                  return Response.json(
                    await prisma.project.findMany({
                      where: {
                        workspaceId: session.user.workspaceId,
                        archived: false,
                      },
                    }),
                  );
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("tenant-scope-missing:")));
}

#[test]
fn detects_jwt_decode_without_verify_in_sensitive_route() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/admin/users/route.ts",
        r#"
                import jwt from "jsonwebtoken";

                export async function POST(request: Request) {
                  const token = request.headers.get("authorization")?.replace("Bearer ", "");
                  const claims = jwt.decode(token!);

                  if ((claims as { role?: string } | null)?.role !== "admin") {
                    return new Response("forbidden", { status: 403 });
                  }

                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("jwt-decode-without-verify:")));
}

#[test]
fn distant_atob_and_filename_split_are_not_a_jwt_decode() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/admin/users/route.ts",
        r#"
                export async function POST(request: Request) {
                  const basic = request.headers.get("authorization")?.replace("Basic ", "");
                  const credentials = atob(basic ?? "");
                  const [user, pass] = credentials.split(":");
                  if (user !== process.env.ADMIN_USER || pass !== process.env.ADMIN_PASS) {
                    return new Response("forbidden", { status: 403 });
                  }

                  const body = await request.json();
                  const fileName: string = body.fileName;
                  const extension = fileName.split(".")[1];
                  return Response.json({ ok: true, extension });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("jwt-decode-without-verify:")),
        "basic-auth atob plus an unrelated filename split is not a JWT decode"
    );
}

#[test]
fn skips_jwt_decode_issue_when_route_verifies_token() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/admin/users/route.ts",
        r#"
                import jwt from "jsonwebtoken";

                export async function POST(request: Request) {
                  const token = request.headers.get("authorization")?.replace("Bearer ", "");
                  const claims = jwt.verify(token!, process.env.JWT_SECRET!) as { role?: string };

                  if (claims.role !== "admin") {
                    return new Response("forbidden", { status: 403 });
                  }

                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("jwt-decode-without-verify:")));
}

#[test]
fn skips_open_redirect_when_server_action_guards_relative_redirects() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/navigation/actions.ts",
        r#"
                "use server";

                import { redirect } from "next/navigation";

                export async function finishFlow(formData: FormData) {
                  const redirectTo = String(formData.get("redirectTo") ?? "/dashboard");
                  if (!redirectTo.startsWith("/")) {
                    redirect("/dashboard");
                  }

                  const target = new URL(redirectTo, process.env.APP_URL);
                  if (target.origin !== process.env.APP_URL) {
                    redirect("/dashboard");
                  }

                  redirect(target.pathname);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("open-redirect:")));
}
