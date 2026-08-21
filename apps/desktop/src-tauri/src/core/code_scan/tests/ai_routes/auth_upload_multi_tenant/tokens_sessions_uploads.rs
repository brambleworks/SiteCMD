use super::super::super::*;

#[test]
fn detects_one_time_token_flow_missing_hash_expiry_and_single_use() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/reset-password/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";

                const prisma = new PrismaClient();

                export async function POST(request: Request) {
                  const body = await request.json();
                  const reset = await prisma.passwordResetToken.findUnique({
                    where: { token: body.token },
                  });

                  if (!reset) {
                    return new Response("not found", { status: 404 });
                  }

                  await prisma.user.update({
                    where: { id: reset.userId },
                    data: { password: body.password },
                  });

                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("one-time-token-raw-lookup:")));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("one-time-token-no-expiry:")));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("one-time-token-no-single-use:")));
}

#[test]
fn creation_timestamps_do_not_count_as_token_expiry_enforcement() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/reset-password/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";

                const prisma = new PrismaClient();

                export async function POST(request: Request) {
                  const body = await request.json();
                  const reset = await prisma.passwordResetToken.findUnique({
                    where: { token: body.token },
                  });

                  if (!reset) {
                    return new Response("not found", { status: 404 });
                  }

                  await prisma.auditLog.create({
                    data: { event: "password-reset", at: new Date(), ts: Date.now() },
                  });

                  await prisma.user.update({
                    where: { id: reset.userId },
                    data: { password: body.password },
                  });

                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("one-time-token-no-expiry:")),
        "creation timestamps are not expiry enforcement"
    );
}

#[test]
fn detects_session_cookie_missing_hardening_flags() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/auth/login/route.ts",
        r#"
                import { cookies } from "next/headers";

                export async function POST() {
                  cookies().set("session-token", "signed-session-token");
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("session-cookie-flags:")));
}

#[test]
fn session_cookie_missing_only_samesite_is_medium_not_high() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/auth/login/route.ts",
        r#"
                import { cookies } from "next/headers";

                export async function POST() {
                  cookies().set("session-token", "signed-session-token", {
                    httpOnly: true,
                    secure: true,
                  });
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("session-cookie-flags:"))
        .expect("expected session-cookie-flags issue");
    assert_eq!(
        issue.severity,
        Severity::Medium,
        "a cookie missing only sameSite must ship the collector's Medium"
    );
}

#[test]
fn public_risk_login_route_without_rate_limit_is_advisory_capped_at_medium() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/auth/login/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  const session = await createSession(body.email, body.password);
                  return Response.json({ token: session.token });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("public-endpoint-rate-limit:"))
        .expect("expected public-endpoint-rate-limit issue");
    assert_eq!(
        issue.severity,
        Severity::Medium,
        "an advisory rate-limit finding must be capped at Medium"
    );
}

#[test]
fn detects_upload_handler_missing_validation_and_scoped_storage_key() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/account/avatar/route.ts",
        r#"
                import { auth } from "@/auth";
                import { createClient } from "@/lib/supabase/server";

                export async function POST(request: Request) {
                  const session = await auth();
                  if (!session?.user?.id) {
                    return new Response("forbidden", { status: 403 });
                  }

                  const formData = await request.formData();
                  const file = formData.get("file") as File;
                  const supabase = createClient();

                  await supabase.storage.from("avatars").upload(file.name, file);

                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("upload-validation:")));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("upload-key-scope:")));
}

#[test]
fn skips_upload_findings_when_handler_validates_and_scopes_file_key() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/account/avatar/route.ts",
        r#"
                import { auth } from "@/auth";
                import { createClient } from "@/lib/supabase/server";

                const MAX_FILE_SIZE = 5 * 1024 * 1024;

                export async function POST(request: Request) {
                  const session = await auth();
                  if (!session?.user?.id || !session.user.workspaceId) {
                    return new Response("forbidden", { status: 403 });
                  }

                  const formData = await request.formData();
                  const file = formData.get("file") as File;
                  if (!file || file.size > MAX_FILE_SIZE) {
                    return new Response("file too large", { status: 400 });
                  }

                  if (!file.type.startsWith("image/")) {
                    return new Response("unsupported type", { status: 400 });
                  }

                  const supabase = createClient();
                  await supabase
                    .storage
                    .from("avatars")
                    .upload(`${session.user.workspaceId}/${session.user.id}/${file.name}`, file, {
                      contentType: file.type,
                    });

                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("upload-validation:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("upload-key-scope:")));
}

#[test]
fn skips_session_cookie_issue_when_flags_are_explicit() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/auth/login/route.ts",
        r#"
                import { cookies } from "next/headers";

                export async function POST() {
                  cookies().set("session-token", "signed-session-token", {
                    httpOnly: true,
                    secure: process.env.NODE_ENV === "production",
                    sameSite: "lax",
                    path: "/",
                  });
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("session-cookie-flags:")));
}

#[test]
fn skips_one_time_token_findings_when_flow_hashes_expires_and_consumes_token() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/reset-password/route.ts",
        r#"
                import { createHash } from "crypto";
                import { PrismaClient } from "@prisma/client";

                const prisma = new PrismaClient();

                export async function POST(request: Request) {
                  const body = await request.json();
                  const tokenHash = createHash("sha256").update(body.token).digest("hex");
                  const reset = await prisma.passwordResetToken.findUnique({
                    where: { tokenHash },
                  });

                  if (!reset || reset.expiresAt < new Date()) {
                    return new Response("invalid token", { status: 400 });
                  }

                  await prisma.user.update({
                    where: { id: reset.userId },
                    data: { password: body.password },
                  });

                  await prisma.passwordResetToken.delete({
                    where: { tokenHash },
                  });

                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("one-time-token-raw-lookup:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("one-time-token-no-expiry:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("one-time-token-no-single-use:")));
}
