//! Route detection and public-endpoint signals: the path-shaped modules that
//! are not routes, the framework gates that sit in front of routes, and the
//! abuse signals that need more than a word match.

use super::*;

fn issue_ids(report: &CodeScanReport) -> Vec<String> {
    report.issues.iter().map(|issue| issue.id.clone()).collect()
}

fn has_issue(report: &CodeScanReport, prefix: &str) -> bool {
    report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with(prefix))
}

fn issue_for<'a>(report: &'a CodeScanReport, id: &str) -> Option<&'a CodeIssue> {
    report.issues.iter().find(|issue| issue.id == id)
}

#[test]
fn modules_under_api_paths_need_a_request_handler_to_count_as_routes() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "next": "^16.0.0" } }"#,
    );
    // A type-only Jest configuration that happens to live under apps/api.
    write_file(
        temp.path(),
        "apps/api/v2/jest.config.ts",
        r#"import type { Config } from "jest";

const config: Config = {
  preset: "ts-jest",
  rootDir: ".",
  testRegex: ".*\\.spec\\.ts$",
};

export default config;
"#,
    );
    // A NestJS service under the same tree: no decorator, no handler.
    write_file(
        temp.path(),
        "apps/api/v2/src/modules/auth/auth.service.ts",
        r#"import { Injectable } from "@nestjs/common";

@Injectable()
export class AuthService {
  getAuthMethods() {
    return ["password", "oauth"];
  }
}
"#,
    );
    // A React Query hook that lives under a features `api/` directory.
    write_file(
        temp.path(),
        "src/features/comments/api/create-comment.ts",
        r#"import { useMutation } from "@tanstack/react-query";

import { api } from "@/lib/api-client";

export const createComment = ({ data }: { data: { body: string } }) =>
  api.post("/comments", data);

export const useCreateComment = () => useMutation({ mutationFn: createComment });
"#,
    );
    // A GET-only static text route.
    write_file(
        temp.path(),
        "apps/docs/src/app/llms.txt/route.ts",
        r#"import { renderLlmsIndex } from "@/lib/llms";

export const dynamic = "force-static";

export function GET() {
  return new Response(renderLlmsIndex());
}
"#,
    );
    // Positive control: a real unauthenticated login handler.
    write_file(
        temp.path(),
        "app/api/login/route.ts",
        r#"export async function POST(request: Request) {
  const body = await request.json();
  return Response.json({ email: body.email });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    for path in [
        "apps/api/v2/jest.config.ts",
        "apps/api/v2/src/modules/auth/auth.service.ts",
        "src/features/comments/api/create-comment.ts",
        "apps/docs/src/app/llms.txt/route.ts",
    ] {
        assert!(
            !ids.iter()
                .any(|id| id == &format!("public-endpoint-rate-limit:{path}")),
            "{path} is not a route, got {ids:?}"
        );
    }
    assert!(
        ids.iter()
            .any(|id| id == "public-endpoint-rate-limit:app/api/login/route.ts"),
        "an unauthenticated login handler keeps its finding, got {ids:?}"
    );
}

#[test]
fn public_endpoint_evidence_names_the_route_marker_and_the_risk_word() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/search/route.ts",
        r#"export async function POST(request: Request) {
  const body = await request.json();
  return Response.json({ hits: body.term });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = issue_for(
        &report,
        "public-endpoint-rate-limit:app/api/search/route.ts",
    )
    .expect("expected the search route finding");
    let evidence = issue.evidence.clone().unwrap_or_default();
    assert!(
        evidence.contains("an exported HTTP verb handler"),
        "evidence names the marker: {evidence}"
    );
    assert!(
        evidence.contains("`search` in the route path"),
        "evidence names the matched risk word: {evidence}"
    );
    // The word lives in the path, so the finding points at the handler rather
    // than at an unrelated line.
    assert_eq!(
        issue.line,
        Some(1),
        "a path-matched risk word anchors on the handler: {issue:?}"
    );
}

#[test]
fn a_layout_route_that_redirects_unauthenticated_callers_protects_its_descendants() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "react-router": "^7.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "app/routes/_authenticated+/_layout.tsx",
        r#"import { Outlet, redirect } from "react-router";

import { getOptionalSession } from "~/auth/session.server";

export async function loader({ request }: { request: Request }) {
  const session = await getOptionalSession(request);

  if (!session.isAuthenticated) {
    throw redirect("/signin");
  }

  return { user: session.user };
}

export default function Layout() {
  return <Outlet />;
}
"#,
    );
    write_file(
        temp.path(),
        "app/routes/_authenticated+/admin+/_layout.tsx",
        r#"import { Outlet, redirect } from "react-router";

import { isAdmin } from "~/auth/roles";
import { getSession } from "~/auth/session.server";

export async function loader({ request }: { request: Request }) {
  const { user } = await getSession(request);

  if (!user || !isAdmin(user)) {
    throw redirect("/");
  }

  return { user };
}

export default function AdminLayout() {
  return <Outlet />;
}
"#,
    );
    write_file(
        temp.path(),
        "app/routes/_authenticated+/admin+/stats.tsx",
        r#"import { getServerSession } from "~/auth/session.server";
import { getAdminStats } from "~/server/admin-stats";

export async function loader({ request }: { request: Request }) {
  const session = await getServerSession(request);
  return { stats: await getAdminStats(), viewer: session.user.email };
}

export default function AdminStatsPage() {
  return <section>Admin billing stats</section>;
}
"#,
    );
    // Positive control: an admin write handler outside the guarded tree.
    write_file(
        temp.path(),
        "app/api/admin/users/route.ts",
        r#"export async function POST(request: Request) {
  const body = await request.json();
  return Response.json({ created: body.email });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    for prefix in [
        "sensitive-auth:",
        "sensitive-authz:",
        "public-endpoint-rate-limit:",
    ] {
        assert!(
            !ids.iter()
                .any(|id| id == &format!("{prefix}app/routes/_authenticated+/admin+/stats.tsx")),
            "the layout loader gates this page, got {ids:?}"
        );
    }
    assert!(
        ids.iter()
            .any(|id| id == "sensitive-auth:app/api/admin/users/route.ts"),
        "an ungated admin handler keeps its finding, got {ids:?}"
    );
}

#[test]
fn transformer_modules_under_api_paths_are_not_sensitive_handlers() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "apps/api/v2/src/platform/transformers/api-to-internal/locations.ts",
        r#"export function transformLocationApiToInternal(location: ApiLocation) {
  return {
    type: location.type,
    address: location.address,
    link: location.link,
  };
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        !ids.iter().any(|id| id.starts_with("sensitive-auth:")),
        "a pure mapping module is neither a route nor an internal endpoint, got {ids:?}"
    );
}

#[test]
fn repository_modules_are_not_unatomic_multi_write_handlers() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "apps/api/v2/src/modules/apps/apps.repository.ts",
        r#"import { Injectable } from "@nestjs/common";

@Injectable()
export class AppsRepository {
  @Post("create")
  async createApp(data: AppInput) {
    return await this.prisma.app.create({ data });
  }

  async updateApp(id: string, data: AppInput) {
    return await this.prisma.app.update({ where: { id }, data });
  }

  async removeApp(id: string) {
    return await this.prisma.app.delete({ where: { id } });
  }
}
"#,
    );
    // Positive control: two awaited writes in one handler body.
    write_file(
        temp.path(),
        "app/api/orders/route.ts",
        r#"export async function POST(request: Request) {
  const body = await request.json();
  await prisma.order.create({ data: body });
  await prisma.auditLog.create({ data: { action: "order.create" } });
  return Response.json({ ok: true });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        !ids.iter()
            .any(|id| id
                == "multi-write-no-transaction:apps/api/v2/src/modules/apps/apps.repository.ts"),
        "one write per repository method is not one unatomic operation, got {ids:?}"
    );
    assert!(
        ids.iter()
            .any(|id| id == "multi-write-no-transaction:app/api/orders/route.ts"),
        "two awaited writes in one handler keep the finding, got {ids:?}"
    );
}

#[test]
fn a_form_field_route_is_not_an_upload_handler() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/routes/api+/locale.tsx",
        r#"import { langCookie } from "~/storage/lang-cookie.server";

export const action = async ({ request }: { request: Request }) => {
  const formData = await request.formData();
  const lang = formData.get("lang") || "";

  return new Response("OK", {
    status: 200,
    headers: { "Set-Cookie": await langCookie.serialize(lang) },
  });
};
"#,
    );
    // Positive control: a real file upload with no size or type guard.
    write_file(
        temp.path(),
        "app/api/attachments/route.ts",
        r#"export async function POST(request: Request) {
  const formData = await request.formData();
  const file = formData.get("file");
  await put(`attachments/${Date.now()}`, file);
  return Response.json({ ok: true });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        !ids.iter()
            .any(|id| id == "upload-validation:app/routes/api+/locale.tsx"),
        "a language form field is not an upload, got {ids:?}"
    );
    assert!(
        ids.iter()
            .any(|id| id == "upload-validation:app/api/attachments/route.ts"),
        "a named file input without guards keeps the finding, got {ids:?}"
    );
}

#[test]
fn request_session_ownership_and_token_scopes_count_as_tenant_scope() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "pages/api/teams/add-calendar.ts",
        r#"export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const user = await prisma.user.findFirstOrThrow({
    where: { id: req.session?.user?.id },
    select: { email: true },
  });
  return res.json({ email: user.email });
}
"#,
    );
    write_file(
        temp.path(),
        "pages/api/teams/add-credential.ts",
        r#"import { throwIfNotHaveAdminAccessToTeam } from "~/lib/teams";

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const teamId = Number(req.query.teamId);
  const credentialOwner = req.query.teamId ? { teamId } : { userId: req.session.user.id };
  await throwIfNotHaveAdminAccessToTeam({ teamId, userId: req.session.user.id });

  const existing = await prisma.credential.findFirst({
    where: { type: "giphy_other", ...credentialOwner },
  });
  return res.json({ existing });
}
"#,
    );
    // The same shape with a spread whose name says nothing about ownership: the
    // binding is what makes it owner-scoped.
    write_file(
        temp.path(),
        "pages/api/teams/add-payment-app.ts",
        r#"export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const teamIdNumber = Number(req.query.teamId);
  const installForObject = teamIdNumber ? { teamId: teamIdNumber } : { userId: req.session.user.id };

  const alreadyInstalled = await prisma.credential.findFirst({
    where: { type: "hitpay_payment", ...installForObject },
  });
  return res.json({ alreadyInstalled });
}
"#,
    );
    // The ownership predicate can ride a write payload instead of a `where`
    // clause: `const data = { ..., userId: session.user?.id }` then
    // `prisma.credential.create({ data })`.
    write_file(
        temp.path(),
        "pages/api/teams/add-exchange.ts",
        r#"export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const session = checkSession(req);
  const data = {
    type: "exchange_calendar",
    teamId: null,
    userId: session.user?.id,
  };

  await prisma.credential.create({ data });
  return res.json({ url: "/apps/installed" });
}
"#,
    );
    // Positive control: an authenticated team route that trusts a record id.
    write_file(
        temp.path(),
        "pages/api/teams/credentials.ts",
        r#"export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const session = await getServerSession(req, res);
  const credentials = await prisma.credential.findMany({ where: { id: req.query.id } });
  return res.json({ credentials, viewer: session.user.email });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    for path in [
        "pages/api/teams/add-calendar.ts",
        "pages/api/teams/add-credential.ts",
        "pages/api/teams/add-payment-app.ts",
        "pages/api/teams/add-exchange.ts",
    ] {
        assert!(
            !ids.iter()
                .any(|id| id == &format!("tenant-scope-missing:{path}")),
            "{path} scopes by the authenticated owner, got {ids:?}"
        );
    }
    // Negative controls on the axis that matters: the payload carries an owner
    // id, but one the caller supplied. Naming the field `userId`, or naming any
    // field after the session, must not read as ownership.
    write_file(
        temp.path(),
        "pages/api/teams/add-integration.ts",
        r#"export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const session = await getServerSession(req, res);
  const data = {
    type: "integration",
    userId: req.body.userId,
  };

  await prisma.credential.create({ data });
  return res.json({ ok: true, viewer: session.user.email });
}
"#,
    );
    // The same forged-ownership payload on a sensitive path: `sensitive-authz`
    // reads the same predicate bit, so it must not be silenced either.
    write_file(
        temp.path(),
        "pages/api/settings/credentials.ts",
        r#"export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const session = await getServerSession(req, res);
  const data = {
    type: "integration",
    userId: req.body.userId,
  };

  await prisma.credential.create({ data });
  return res.json({ ok: true, viewer: session.user.email });
}
"#,
    );
    write_file(
        temp.path(),
        "pages/api/teams/add-connector.ts",
        r#"export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const session = await getServerSession(req, res);
  const data = {
    type: "connector",
    teamId: req.body.teamId,
    sessionLabel: req.body.label,
    currentUserNote: req.body.note,
  };

  await prisma.credential.create({ data });
  return res.json({ ok: true, viewer: session.user.email });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        ids.iter()
            .any(|id| id == "tenant-scope-missing:pages/api/teams/credentials.ts"),
        "a record-id lookup in a team route keeps the finding, got {ids:?}"
    );
    for path in [
        "pages/api/teams/add-integration.ts",
        "pages/api/teams/add-connector.ts",
    ] {
        assert!(
            ids.iter()
                .any(|id| id == &format!("tenant-scope-missing:{path}")),
            "{path} takes its owner id from the request, got {ids:?}"
        );
    }
    assert!(
        ids.iter()
            .any(|id| id == "sensitive-authz:pages/api/settings/credentials.ts"),
        "a forged owner id is not an authorization decision either, got {ids:?}"
    );
}

#[test]
fn webhook_verification_in_a_guard_or_provider_helper_is_recognized() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "apps/api/src/webhooks/vercel-webhook.controller.ts",
        r#"import { Controller, Post, UseGuards } from "@nestjs/common";

import { VercelWebhookGuard } from "./vercel-webhook.guard";

@Controller({ path: "/v2/webhooks/vercel" })
export class VercelWebhookController {
  @Post("deployment-promoted")
  @UseGuards(VercelWebhookGuard)
  async handlePromotion(@Body() body: VercelWebhookPayload) {
    return { received: body.type };
  }
}
"#,
    );
    write_file(
        temp.path(),
        "pages/api/webhooks/alby.ts",
        r#"import parseInvoice from "~/app-store/alby/lib/parseInvoice";

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const bodyAsString = (await getRawBody(req)).toString();
  const invoice = await parseInvoice(bodyAsString, req.headers, credentials.webhook_endpoint_secret);
  return res.json({ settled: invoice.settled });
}
"#,
    );
    // Positive controls: a sender and a logger that both carry a body argument
    // and a secret nearby, and a handler that verifies nothing at all.
    write_file(
        temp.path(),
        "app/api/webhooks/forward/route.ts",
        r#"export async function POST(request: Request) {
  const payload = await request.json();
  await postWebhook(payload, { url: process.env.MIRROR_URL, secret: process.env.MIRROR_SECRET });
  return Response.json({ ok: true });
}
"#,
    );
    write_file(
        temp.path(),
        "app/api/webhooks/audit/route.ts",
        r#"export async function POST(request: Request) {
  const rawBody = await request.text();
  await logDelivery(rawBody, cfg.secretRef);
  return Response.json({ ok: true });
}
"#,
    );
    write_file(
        temp.path(),
        "app/api/webhooks/unverified/route.ts",
        r#"export async function POST(request: Request) {
  const body = await request.json();
  await prisma.event.create({ data: { kind: body.type } });
  return Response.json({ ok: true });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    for path in [
        "apps/api/src/webhooks/vercel-webhook.controller.ts",
        "pages/api/webhooks/alby.ts",
    ] {
        assert!(
            !ids.iter()
                .any(|id| id == &format!("webhook-signature:{path}")),
            "{path} verifies the delivery, got {ids:?}"
        );
    }
    for path in [
        "app/api/webhooks/unverified/route.ts",
        "app/api/webhooks/forward/route.ts",
        "app/api/webhooks/audit/route.ts",
    ] {
        assert!(
            ids.iter()
                .any(|id| id == &format!("webhook-signature:{path}")),
            "{path} verifies nothing, got {ids:?}"
        );
    }
}

#[test]
fn a_stripe_signature_check_and_a_bundled_asset_load_are_not_outbound_calls() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/webhooks/stripe/route.ts",
        r#"import { headers } from "next/headers";

import { stripe } from "@/lib/stripe";

export async function POST(req: Request) {
  const body = await req.text();
  const signature = headers().get("Stripe-Signature") as string;
  const event = stripe.webhooks.constructEvent(body, signature, process.env.STRIPE_WEBHOOK_SECRET);
  return new Response(JSON.stringify({ received: event.type }));
}
"#,
    );
    write_file(
        temp.path(),
        "app/api/og/route.tsx",
        r#"import { ImageResponse } from "@vercel/og";

const interRegular = fetch(new URL("../../../assets/fonts/Inter-Regular.ttf", import.meta.url)).then(
  (res) => res.arrayBuffer(),
);

export async function POST(req: Request) {
  const body = await req.json();
  return new ImageResponse(<div>{body.heading}</div>, {
    fonts: [{ name: "Inter", data: await interRegular }],
  });
}
"#,
    );
    // Positive control: an unauthenticated read-only proxy. Its name carries no
    // abuse-sensitive word, and it still needs a deadline.
    write_file(
        temp.path(),
        "app/api/community/total-forks/route.ts",
        r#"export async function GET(request: Request) {
  const res = await fetch("https://stats.example.com/api/stats");
  const data = await res.json();
  return Response.json(data);
}
"#,
    );
    // Positive control: a genuine remote call on a public route.
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"export async function POST(request: Request) {
  const body = await request.json();
  await fetch("https://hooks.example.com/notify", { method: "POST", body: JSON.stringify(body) });
  return Response.json({ ok: true });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    for path in ["app/api/webhooks/stripe/route.ts", "app/api/og/route.tsx"] {
        for prefix in ["external-call-timeout:", "external-call-retry:"] {
            assert!(
                !ids.iter().any(|id| id == &format!("{prefix}{path}")),
                "{path} makes no outbound call, got {ids:?}"
            );
        }
    }
    for path in [
        "app/api/contact/route.ts",
        "app/api/community/total-forks/route.ts",
    ] {
        assert!(
            ids.iter()
                .any(|id| id == &format!("external-call-timeout:{path}")),
            "{path} makes an unguarded remote call, got {ids:?}"
        );
    }
}

#[test]
fn a_verified_webhook_is_not_asked_to_scope_by_tenant_or_to_authorize_a_user() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/teams/webhook/route.ts",
        r#"import { createHmac, timingSafeEqual } from "node:crypto";

export async function POST(request: Request) {
  const body = await request.text();
  const signature = request.headers.get("x-hub-signature") ?? "";
  const expected = createHmac("sha256", process.env.TEAM_WEBHOOK_SECRET ?? "")
    .update(body)
    .digest("hex");

  if (!timingSafeEqual(Buffer.from(signature), Buffer.from(expected))) {
    return new Response("bad signature", { status: 400 });
  }

  const session = await getServerSession();
  const payload = JSON.parse(body);
  await prisma.booking.update({ where: { id: payload.bookingId }, data: { status: "done" } });
  return Response.json({ ok: true, admin: session?.user?.email });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    for prefix in ["tenant-scope-missing:", "sensitive-authz:"] {
        assert!(
            !ids.iter()
                .any(|id| id == &format!("{prefix}app/api/teams/webhook/route.ts")),
            "a signature-verified webhook has no user to scope or authorize, got {ids:?}"
        );
    }
}

#[test]
fn orm_model_calls_are_not_express_route_declarations() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "packages/prisma/delete-app.ts",
        r#"import prisma from ".";

async function main() {
  const appId = process.argv[2];
  await prisma.app.delete({ where: { slug: appId } });
  await prisma.credential.deleteMany({ where: { appId } });
}

main();
"#,
    );
    // Positive control: a real Express delete route.
    write_file(
        temp.path(),
        "src/server.ts",
        r#"import express from "express";

const app = express();

app.delete("/items/:id", async (req, res) => {
  await prisma.item.delete({ where: { id: req.params.id } });
  await prisma.auditLog.create({ data: { action: "item.delete" } });
  res.json({ ok: true });
});
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    for prefix in ["db-in-route:", "multi-write-no-transaction:"] {
        assert!(
            !ids.iter()
                .any(|id| id == &format!("{prefix}packages/prisma/delete-app.ts")),
            "a maintenance script is not a route, got {ids:?}"
        );
    }
    assert!(
        has_issue(&report, "multi-write-no-transaction:src/server.ts"),
        "a real Express route keeps its findings, got {ids:?}"
    );
}

#[test]
fn test_support_and_sample_trees_are_not_scanned_for_issues() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/testing/test-utils.tsx",
        r#"import { db } from "./mocks/db";

export const createUser = (user: { password: string }) => {
  return db.user.create({ ...user, password: hash(user.password) });
};
"#,
    );
    write_file(
        temp.path(),
        "sample/19-auth-jwt/src/users/users.service.ts",
        r#"export class UsersService {
  private readonly users = [{ userId: 1, username: "john", password: "changeme" }];

  async findOne(username: string) {
    return this.users.find((user) => user.username === username);
  }
}
"#,
    );
    write_file(
        temp.path(),
        "packages/users/UserRepository.integration-test.ts",
        r#"describe("UserRepository", () => {
  it("creates a user", async () => {
    const password = await bcrypt.hash("password123", 10);
    expect(password).toBeDefined();
  });
});
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    for prefix in ["plaintext-password:", "weak-default-credential:"] {
        assert!(
            !ids.iter().any(|id| id.starts_with(prefix)),
            "test-support and sample trees carry no shipped credentials, got {ids:?}"
        );
    }
}

#[test]
fn a_nest_body_dto_counts_as_request_validation() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "apps/api/src/bookings/bookings.controller.ts",
        r#"import { Body, Controller, Post, Req } from "@nestjs/common";

import { CreateBookingInput } from "./inputs/create-booking.input";

@Controller("bookings")
export class BookingsController {
  @Post("/")
  async createBooking(@Body() body: CreateBookingInput, @Req() req: Request) {
    return { start: body.start, source: req.body.source };
  }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        !ids.iter().any(|id| id.starts_with("request-validation:")),
        "a typed Nest body parameter is validated by its DTO, got {ids:?}"
    );
}

#[test]
fn a_shorthand_token_lookup_is_a_raw_one_time_token_lookup() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/routes/_unauthenticated+/organisation.invite.$token.tsx",
        r#"export async function loader({ params }: Route.LoaderArgs) {
  const token = params.token;

  const invite = await prisma.organisationMemberInvite.findUnique({
    where: {
      token,
    },
  });

  return { invite };
}
"#,
    );
    // A handler that derives a digest before the lookup must stay clean.
    write_file(
        temp.path(),
        "app/routes/_unauthenticated+/password-reset.$token.tsx",
        r#"import { createHash } from "node:crypto";

export async function loader({ params }: Route.LoaderArgs) {
  const hashed = createHash("sha256").update(params.token).digest("hex");

  const reset = await prisma.passwordResetToken.findUnique({
    where: { hashedToken: hashed, expiresAt: { gt: new Date() } },
  });

  return { reset };
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        ids.iter().any(|id| id
            == "one-time-token-raw-lookup:app/routes/_unauthenticated+/organisation.invite.$token.tsx"),
        "a shorthand token predicate is the same raw lookup, got {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id
            == "one-time-token-raw-lookup:app/routes/_unauthenticated+/password-reset.$token.tsx"),
        "a hashed lookup is not a raw one, got {ids:?}"
    );
}

#[test]
fn a_named_oauth_state_decoder_is_a_state_binding() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "pages/api/integrations/feishu/callback.ts",
        r#"import { decodeOAuthState } from "~/app-store/_utils/oauth/decodeOAuthState";

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const code = req.query.code;
  const state = decodeOAuthState(req);

  const response = await fetch("https://open.feishu.cn/open-apis/authen/v1/access_token", {
    method: "POST",
    body: JSON.stringify({ grant_type: "authorization_code", code }),
  });

  return res.redirect(state?.returnTo ?? "/apps/installed");
}
"#,
    );
    // Positive control: the same exchange with no state handling at all.
    write_file(
        temp.path(),
        "pages/api/integrations/other/callback.ts",
        r#"export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const code = req.query.code;

  const response = await fetch("https://provider.example.com/oauth/access_token", {
    method: "POST",
    body: JSON.stringify({ grant_type: "authorization_code", code }),
  });

  return res.redirect("/apps/installed");
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        !ids.iter()
            .any(|id| id == "oauth-callback-state:pages/api/integrations/feishu/callback.ts"),
        "a named state decoder carries the browser binding, got {ids:?}"
    );
    assert!(
        ids.iter()
            .any(|id| id == "oauth-callback-state:pages/api/integrations/other/callback.ts"),
        "a callback with no state handling keeps the finding, got {ids:?}"
    );
}

#[test]
fn a_revalidate_only_server_action_is_not_a_sensitive_handler() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/settings/developer/api-keys/actions.ts",
        r#""use server";

import { revalidateTag } from "next/cache";

export async function revalidateApiKeysList() {
  revalidateTag("viewer.apiKeys.list");
}
"#,
    );
    // Positive control: the same directory, but the action writes.
    write_file(
        temp.path(),
        "app/settings/developer/api-keys/delete.ts",
        r#""use server";

import { createAdminClient } from "@/lib/supabase/admin";

export async function deleteApiKey(id: string) {
  const admin = createAdminClient();
  await admin.from("api_keys").delete().eq("id", id);
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        !ids.iter()
            .any(|id| id == "sensitive-auth:app/settings/developer/api-keys/actions.ts"),
        "cache revalidation is the whole body, got {ids:?}"
    );
    assert!(
        ids.iter()
            .any(|id| id == "sensitive-auth:app/settings/developer/api-keys/delete.ts"),
        "an ungated admin write keeps the finding, got {ids:?}"
    );
}

#[test]
fn unawaited_writes_still_classify_the_handler_as_touching_the_database() {
    let temp = TempDir::new().unwrap();
    // Neither write is awaited, and neither handler runs a read query, so the
    // write count is the only signal that the handler reaches a database. A
    // count of zero would erase the tenant handler and the server action alike.
    write_file(
        temp.path(),
        "app/api/teams/purge/route.ts",
        r#"export async function POST(request: Request) {
  const session = await getServerSession();
  const body = await request.json();

  prisma.report.delete({ where: { id: body.id } });
  prisma.auditLog.deleteMany({ where: { reportId: body.id } });

  return Response.json({ ok: true, viewer: session.user.email });
}
"#,
    );
    write_file(
        temp.path(),
        "app/settings/reports/purge.ts",
        r#""use server";

import { createAdminClient } from "@/lib/supabase/admin";

export async function purgeReports(id: string) {
  const admin = createAdminClient();
  admin.from("reports").delete().eq("id", id);
  admin.from("audit_log").delete().eq("report_id", id);
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        ids.iter()
            .any(|id| id == "tenant-scope-missing:app/api/teams/purge/route.ts"),
        "an unscoped team handler that writes keeps the finding, got {ids:?}"
    );
    assert!(
        ids.iter()
            .any(|id| id == "sensitive-auth:app/settings/reports/purge.ts"),
        "an ungated admin action that writes keeps the finding, got {ids:?}"
    );
}

#[test]
fn a_handler_reached_through_a_factory_is_still_a_route() {
    let temp = TempDir::new().unwrap();
    // cal.com's app-store handlers export their router through a factory rather
    // than as a named function, and they live outside `pages/api/`.
    write_file(
        temp.path(),
        "packages/app-store/sendgrid/api/check.ts",
        r#"import type { NextApiRequest } from "next";

import { defaultHandler } from "@calcom/lib/server/defaultHandler";
import { defaultResponder } from "@calcom/lib/server/defaultResponder";

export async function getHandler(req: NextApiRequest) {
  const { api_key } = req.body;
  const usage = await fetch(`https://api.sendgrid.com/v3/user/username?key=${api_key}`);
  return usage.json();
}

export default defaultHandler({
  GET: Promise.resolve({ default: defaultResponder(getHandler) }),
});
"#,
    );
    // Negative control: a default export of a call that names no handler role.
    write_file(
        temp.path(),
        "packages/app-store/sendgrid/api/config.ts",
        r#"import { defineAppConfig } from "@calcom/app-store/config";

export default defineAppConfig({ name: "sendgrid", categories: ["automation"] });
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        ids.iter()
            .any(|id| id == "external-call-timeout:packages/app-store/sendgrid/api/check.ts"),
        "a factory-exported handler is a route, got {ids:?}"
    );
    assert!(
        !ids.iter().any(
            |id| id.ends_with(":packages/app-store/sendgrid/api/config.ts")
                && id.starts_with("external-call")
        ),
        "an app config is not a route, got {ids:?}"
    );
}
