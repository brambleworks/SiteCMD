//! Project-level cases for signals that must read the code, not the prose
//! around it, and for the sink refinements that need the value beside a match.

use super::*;

fn ids_starting_with<'a>(report: &'a CodeScanReport, slug: &str) -> Vec<&'a str> {
    let prefix = format!("{slug}:");
    report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with(&prefix))
        .map(|issue| issue.id.as_str())
        .collect()
}

fn assert_absent(report: &CodeScanReport, slug: &str) {
    let found = ids_starting_with(report, slug);
    assert!(
        found.is_empty(),
        "expected no {slug} finding, got {found:?}"
    );
}

fn assert_present(report: &CodeScanReport, slug: &str) {
    let found = ids_starting_with(report, slug);
    assert!(!found.is_empty(), "expected a {slug} finding, got none");
}

#[test]
fn commented_out_queries_produce_no_route_findings() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "commented-app" }"#,
    );
    write_file(
        temp.path(),
        "app/api/settings/route.ts",
        r#"import { prisma } from "@/lib/prisma";

export async function GET(request: Request) {
  const session = await getSession();
  // Legacy lookup, kept for reference:
  //   const accounts = await prisma.account.findMany({
  //     where: { userId: session.user.id },
  //   });
  /* Older shape:
     await prisma.account.findMany();
  */
  return Response.json({ ok: true, user: session.user.id });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "no-pagination");
    assert_absent(&report, "sensitive-authz");
}

#[test]
fn a_live_unpaginated_query_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "live-app" }"#);
    write_file(
        temp.path(),
        "app/api/settings/route.ts",
        r#"import { prisma } from "@/lib/prisma";

export async function GET(request: Request) {
  const session = await getSession();
  const accounts = await prisma.account.findMany({ where: { userId: session.user.id } });
  return Response.json({ accounts });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "no-pagination");
}

#[test]
fn search_params_get_all_is_not_a_list_query() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "params-app" }"#);
    write_file(
        temp.path(),
        "app/api/multisign/route.ts",
        r#"export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const tokens = searchParams.getAll("token");
  return Response.json({ count: tokens.length });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "no-pagination");
}

#[test]
fn documented_sinks_and_literal_values_are_not_unsafe_html() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "html-app" }"#);
    write_file(
        temp.path(),
        "src/field-types.ts",
        r#"/**
 * Once a component supports labelAsSafeHtml, add its field type here.
 * A whitelist is needed because unless we use dangerouslySetInnerHTML,
 * React escapes the markup.
 */
export const fieldTypes = ["boolean", "radio"];
"#,
    );
    write_file(
        temp.path(),
        "src/reset-motion.tsx",
        r#"export function ResetMotion() {
  return <style dangerouslySetInnerHTML={{ __html: `* { animation: none !important; }` }} />;
}
"#,
    );
    write_file(
        temp.path(),
        "src/markdown-view.tsx",
        r#"import { markdownToSafeHTML } from "./markdown";

export function MarkdownView({ source }) {
  return <div dangerouslySetInnerHTML={{ __html: markdownToSafeHTML(source.content) }} />;
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "unsafe-html");
}

#[test]
fn an_interpolated_raw_html_value_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "html-sink-app" }"#,
    );
    write_file(
        temp.path(),
        "src/bio.tsx",
        r#"export function Bio({ user }) {
  return <div dangerouslySetInnerHTML={{ __html: user.bio }} />;
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "unsafe-html");
}

#[test]
fn a_credential_derived_fetch_url_is_not_request_controlled() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "fetch-app" }"#);
    write_file(
        temp.path(),
        "app/api/projects/route.ts",
        r#"export async function GET(request: Request) {
  const credential = await getCredential();
  const url = `${credential.account.href}/projects.json`;
  const response = await fetch(url);
  return Response.json(await response.json());
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "user-controlled-fetch");
}

#[test]
fn a_request_bound_fetch_url_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "ssrf-app" }"#);
    write_file(
        temp.path(),
        "app/api/proxy/route.ts",
        r#"export async function POST(request: Request) {
  const body = await request.json();
  const url = body.url;
  const response = await fetch(url);
  return Response.json(await response.json());
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "user-controlled-fetch");
}

#[test]
fn a_named_return_to_guard_clears_open_redirect() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "signin-app" }"#);
    write_file(
        temp.path(),
        "app/api/signin/route.ts",
        r#"import { isValidReturnTo, normalizeReturnTo } from "@/lib/return-to";

export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const returnTo = searchParams.get("returnTo");
  const target = isValidReturnTo(returnTo) ? normalizeReturnTo(returnTo) : "/";
  return Response.redirect(target);
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "open-redirect");
}

#[test]
fn an_unguarded_return_to_still_reports_open_redirect() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "redirect-app" }"#);
    write_file(
        temp.path(),
        "app/api/signin/route.ts",
        r#"export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  return Response.redirect(searchParams.get("returnTo"));
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "open-redirect");
}

#[test]
fn an_authorization_server_token_endpoint_is_not_a_client_callback() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "oauth-server" }"#);
    write_file(
        temp.path(),
        "app/api/oauth/callback/route.ts",
        r#"export async function POST(request: Request) {
  const body = await request.json();
  const tokens = await exchangeCodeForTokens(body.client_id, body.code, body.client_secret, {
    grant_type: "authorization_code",
  });
  return Response.json(tokens);
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "oauth-callback-state");
    assert_absent(&report, "oauth-callback-pkce");
}

#[test]
fn a_client_oauth_callback_without_state_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "oauth-client" }"#);
    write_file(
        temp.path(),
        "app/api/oauth/callback/route.ts",
        r#"export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const code = searchParams.get("code");
  const tokens = await fetch("https://provider.example/oauth/token", {
    method: "POST",
    body: JSON.stringify({ code, grant_type: "authorization_code" }),
  });
  return Response.json(await tokens.json());
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "oauth-callback-state");
}

#[test]
fn a_hashed_password_write_is_not_plaintext_storage() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "signup-app" }"#);
    write_file(
        temp.path(),
        "src/register.ts",
        r#"import { hash } from "./crypto";

export function createUser(db, user) {
  return db.user.create({ data: { ...user, password: hash(user.password) } });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "plaintext-password");
}

#[test]
fn an_unhashed_password_write_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "plaintext-app" }"#,
    );
    write_file(
        temp.path(),
        "src/register.ts",
        r#"export function createUser(db, body) {
  return db.user.create({ data: { email: body.email, password: body.password } });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "plaintext-password");
}

#[test]
fn a_weak_literal_being_hashed_is_not_a_default_credential() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "seed-app" }"#);
    write_file(
        temp.path(),
        "src/seed.ts",
        r#"import bcrypt from "bcryptjs";

export async function seedUser(db) {
  await db.user.create({
    data: { email: "seed@example.test", passwordHash: await bcrypt.hash("password123", 10) },
  });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "weak-default-credential");
}

#[test]
fn a_shipped_default_credential_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "default-cred-app" }"#,
    );
    write_file(
        temp.path(),
        "src/config.ts",
        r#"export const adminPassword = process.env.ADMIN_PASSWORD || "changeme";
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "weak-default-credential");
}

#[test]
fn a_turnstile_sitekey_is_not_a_client_env_secret() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "captcha-app" }"#);
    write_file(
        temp.path(),
        "src/constants.ts",
        r#"export const TURNSTILE_SITE_KEY = process.env.NEXT_PUBLIC_CLOUDFLARE_SITEKEY;
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "client-env-secret");
}

#[test]
fn a_canvas_node_lookup_in_a_loop_is_not_a_database_query() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "canvas-app" }"#);
    write_file(
        temp.path(),
        "src/render-pages.ts",
        r#"export function drawPages(pages, layer) {
  for (const page of pages) {
    const node = layer.current.findOne('#page-' + page.id);
    node.draw();
  }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "n-plus-one-query");
}

#[test]
fn a_per_iteration_orm_lookup_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "nplus1-app" }"#);
    write_file(
        temp.path(),
        "src/load-authors.ts",
        r#"export async function loadAuthors(posts, prisma) {
  for (const post of posts) {
    const author = await prisma.user.findUnique({ where: { id: post.authorId } });
    post.author = author;
  }
  return posts;
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "n-plus-one-query");
}

#[test]
fn an_entity_with_two_owner_ids_is_not_a_join_table() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

enum EnvelopeStatus {
  DRAFT
  SENT
}

model Envelope {
  id        String         @id
  title     String
  status    EnvelopeStatus
  qrToken   String
  userId    String
  teamId    String
  createdAt DateTime       @default(now())
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "schema-join-missing-composite-unique");
    assert_absent(&report, "schema-join-nullable-relations");
    assert_absent(&report, "schema-join-missing-delete-intent");
}

#[test]
fn a_pivot_model_without_a_composite_key_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

enum Role {
  OWNER
  MEMBER
}

model TeamMember {
  id     String @id
  userId String
  teamId String
  role   Role
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "schema-join-missing-composite-unique");
}

#[test]
fn an_integration_suite_tsconfig_is_not_the_build_type_policy() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "suite-app" }"#);
    write_file(
        temp.path(),
        "integration/hello-world/tsconfig.json",
        r#"{ "compilerOptions": { "noImplicitAny": false } }"#,
    );
    write_file(
        temp.path(),
        "example-apps/demo/tsconfig.json",
        r#"{ "compilerOptions": { "strict": false } }"#,
    );
    write_file(
        temp.path(),
        "tsconfig.json",
        r#"{ "compilerOptions": { "strict": false } }"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let found = ids_starting_with(&report, "tsconfig-strict-off");
    assert_eq!(
        found,
        vec!["tsconfig-strict-off:tsconfig.json"],
        "only the project's own tsconfig should report"
    );
}

#[test]
fn a_wordpress_plugin_owns_no_migration_directory() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "wp-plugin-e2e", "private": true }"#,
    );
    write_file(
        temp.path(),
        "sample-importer.php",
        r#"<?php
/*
 * Plugin Name:       Sample Importer
 * Description:       Imports posts from an export file.
 * Version:           0.9.6
 */

function sample_importer_store( $row ) {
    global $wpdb;
    $wpdb->insert( $wpdb->posts, $row );
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "migration-workflow-missing");
}

#[test]
fn a_published_package_repository_owns_no_migration_directory() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "@scope/framework", "workspaces": ["packages/*"], "devDependencies": { "typeorm": "^0.3.20" } }"#,
    );
    write_file(
        temp.path(),
        "packages/core/package.json",
        r#"{ "name": "@scope/core", "main": "./index.js" }"#,
    );
    write_file(
        temp.path(),
        "packages/core/index.ts",
        r#"export function createApplication() {
  return { start: () => undefined };
}
"#,
    );
    write_file(
        temp.path(),
        "integration/typeorm/src/app.service.ts",
        r#"import { getConnection } from "typeorm";

export async function listPhotos() {
  return getConnection().getRepository("Photo").find();
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "migration-workflow-missing");
}

#[test]
fn a_database_backed_application_still_reports_a_missing_migration_workflow() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "posts-app", "dependencies": { "@prisma/client": "^6.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "app/api/posts/route.ts",
        r#"import { prisma } from "@/lib/prisma";

export async function POST(request: Request) {
  const body = await request.json();
  const post = await prisma.post.create({ data: { title: body.title } });
  return Response.json({ post });
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "migration-workflow-missing");
}

#[test]
fn an_og_image_route_keeps_its_inline_styles() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "og-app" }"#);
    write_file(
        temp.path(),
        "app/api/og/route.tsx",
        r##"import { ImageResponse } from "next/og";

export async function GET() {
  return new ImageResponse(
    (
      <div style={{ display: "flex", background: "#ffffff", width: "100%" }}>
        <div style={{ fontSize: 60, color: "#000000" }}>Hello</div>
        <div style={{ marginTop: 20, color: "#333333" }}>World</div>
        <div style={{ padding: 12, color: "#111111" }}>Again</div>
      </div>
    ),
    { width: 1200, height: 630 },
  );
}
"##,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "jsx-inline-style-density");
}

#[test]
fn catch_blocks_that_document_the_ignored_failure_are_not_empty() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "observer-app" }"#);
    write_file(
        temp.path(),
        "src/observe.js",
        r#"export function observe(target) {
  try {
    target.observe({ type: "layout-shift" });
  } catch (e) {
    // Unsupported entry types leave this optional metric unset.
  }
  try {
    target.observe({ type: "largest-contentful-paint" });
  } catch (e) {
    // Unsupported entry types leave this optional metric unset.
  }
  try {
    target.observe({ type: "event" });
  } catch (e) {
    // Unsupported entry types leave this optional metric unset.
  }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_absent(&report, "empty-catch-blocks");
}

#[test]
fn catch_blocks_that_say_nothing_at_all_still_report() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "silent-app" }"#);
    write_file(
        temp.path(),
        "src/silent.js",
        r#"export function silent(target) {
  try {
    target.a();
  } catch (e) {}
  try {
    target.b();
  } catch (e) {}
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "empty-catch-blocks");
}

#[test]
fn a_destructured_request_url_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "destructure-app" }"#,
    );
    write_file(
        temp.path(),
        "app/api/proxy/route.ts",
        r#"export async function POST(request: Request) {
  const { url } = await request.json();
  const response = await fetch(url);
  return Response.json(await response.json());
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "user-controlled-fetch");
}

#[test]
fn a_concatenated_raw_html_value_still_reports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "concat-html-app" }"#,
    );
    write_file(
        temp.path(),
        "src/bio.tsx",
        r#"export function Bio({ user }) {
  return <div dangerouslySetInnerHTML={{ __html: "<b>" + user.bio }} />;
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "unsafe-html");
}

#[test]
fn a_value_named_unsafe_html_is_not_sanitization() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "unsafe-html-app" }"#,
    );
    write_file(
        temp.path(),
        "src/preview.tsx",
        r#"export function Preview({ post }) {
  const unsafeHtml = post.body;
  return <div dangerouslySetInnerHTML={{ __html: unsafeHtml }} />;
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "unsafe-html");
}

#[test]
fn a_routeless_database_backed_worker_still_reports_a_missing_migration_workflow() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "digest-worker", "main": "./dist/index.js", "dependencies": { "@prisma/client": "^6.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "src/worker.ts",
        r#"import { prisma } from "./prisma";

export async function runDigest() {
  const users = await prisma.user.findMany({ take: 100 });
  for (const user of users) {
    await prisma.digest.create({ data: { userId: user.id } });
  }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "migration-workflow-missing");
}

#[test]
fn a_monorepo_that_merely_contains_a_wordpress_plugin_keeps_the_check() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "agency-monorepo", "dependencies": { "@prisma/client": "^6.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "plugins/importer/importer.php",
        r#"<?php
/*
 * Plugin Name:       Importer
 */

function importer_store( $row ) {
    global $wpdb;
    $wpdb->insert( $wpdb->posts, $row );
}
"#,
    );
    write_file(
        temp.path(),
        "app/api/posts/route.ts",
        r#"import { prisma } from "@/lib/prisma";

export async function POST(request: Request) {
  const body = await request.json();
  return Response.json(await prisma.post.create({ data: { title: body.title } }));
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert_present(&report, "migration-workflow-missing");
}
