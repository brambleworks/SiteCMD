use super::super::*;

#[test]
fn detects_stripe_checkout_with_user_controlled_price_and_no_idempotency() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/billing/route.ts",
        r#"
                import Stripe from "stripe";

                const stripe = new Stripe(process.env.STRIPE_SECRET!, { apiVersion: "2024-06-20" });

                export async function POST(request: Request) {
                  const body = await request.json();
                  const session = await stripe.checkout.sessions.create({
                    mode: "subscription",
                    line_items: [{ price: body.priceId, quantity: 1 }],
                    success_url: "https://example.com/success",
                    cancel_url: "https://example.com/cancel",
                  });

                  return Response.json({ id: session.id });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let price_issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("stripe-user-controlled-price:"))
        .expect("stripe-user-controlled-price issue"); // allow-expect: test assertion
    assert!(price_issue.title.contains("Possible"));
    assert!(price_issue.description.contains("does not establish"));
    assert!(price_issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("server-owned"));
    let idempotency_issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("stripe-checkout-idempotency:"))
        .expect("stripe-checkout-idempotency issue"); // allow-expect: test assertion
    assert!(idempotency_issue.title.contains("No recognized"));
    assert!(idempotency_issue.description.contains("does not establish"));
}

#[test]
fn skips_stripe_checkout_findings_when_price_is_allowlisted_and_idempotent() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/billing/route.ts",
        r#"
                import { z } from "zod";
                import Stripe from "stripe";

                const stripe = new Stripe(process.env.STRIPE_SECRET!, { apiVersion: "2024-06-20" });
                const PRICE_MAP = {
                  starter: "price_starter",
                  pro: "price_pro",
                } as const;

                export async function POST(request: Request) {
                  const body = z.object({
                    plan: z.enum(["starter", "pro"]),
                    requestId: z.string().min(8),
                  }).parse(await request.json());

                  const session = await stripe.checkout.sessions.create({
                    mode: "subscription",
                    line_items: [{ price: PRICE_MAP[body.plan], quantity: 1 }],
                    success_url: "https://example.com/success",
                    cancel_url: "https://example.com/cancel",
                  }, {
                    idempotencyKey: body.requestId,
                  });

                  return Response.json({ id: session.id });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("stripe-user-controlled-price:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("stripe-checkout-idempotency:")));
}

#[test]
fn detects_open_redirect_from_user_controlled_server_action() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/navigation/actions.ts",
        r#"
                "use server";

                import { redirect } from "next/navigation";

                export async function finishFlow(formData: FormData) {
                  const redirectTo = formData.get("redirectTo");
                  redirect(redirectTo as string);
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("open-redirect:")));
}

#[test]
fn detects_oauth_callback_missing_state_validation() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/auth/callback/route.ts",
        r#"
                export async function GET(request: Request) {
                  const { searchParams } = new URL(request.url);
                  const code = searchParams.get("code");

                  const tokenResponse = await fetch("https://github.com/login/oauth/access_token", {
                    method: "POST",
                    headers: { "content-type": "application/x-www-form-urlencoded" },
                    body: new URLSearchParams({
                      client_id: process.env.GITHUB_CLIENT_ID!,
                      client_secret: process.env.GITHUB_CLIENT_SECRET!,
                      code: code!,
                      redirect_uri: process.env.APP_URL! + "/api/auth/callback",
                      grant_type: "authorization_code",
                    }),
                  });

                  return Response.json({ ok: tokenResponse.ok });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("oauth-callback-state:")));
}

#[test]
fn skips_oauth_state_issue_when_callback_validates_state() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/auth/callback/route.ts",
        r#"
                import { cookies } from "next/headers";

                export async function GET(request: Request) {
                  const { searchParams } = new URL(request.url);
                  const code = searchParams.get("code");
                  const state = searchParams.get("state");
                  const expectedState = cookies().get("oauth_state")?.value;

                  if (!state || state !== expectedState) {
                    return new Response("invalid state", { status: 400 });
                  }

                  const tokenResponse = await fetch("https://github.com/login/oauth/access_token", {
                    method: "POST",
                    headers: { "content-type": "application/x-www-form-urlencoded" },
                    body: new URLSearchParams({
                      client_id: process.env.GITHUB_CLIENT_ID!,
                      client_secret: process.env.GITHUB_CLIENT_SECRET!,
                      code: code!,
                      redirect_uri: process.env.APP_URL! + "/api/auth/callback",
                      grant_type: "authorization_code",
                    }),
                  });

                  return Response.json({ ok: tokenResponse.ok });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("oauth-callback-state:")));
}

#[test]
fn detects_oauth_callback_missing_pkce_for_public_client_style_exchange() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/auth/callback/route.ts",
        r#"
                export async function GET(request: Request) {
                  const { searchParams } = new URL(request.url);
                  const code = searchParams.get("code");
                  const state = searchParams.get("state");
                  const expectedState = "known";

                  if (!state || state !== expectedState) {
                    return new Response("invalid state", { status: 400 });
                  }

                  const tokenResponse = await fetch("https://oauth2.googleapis.com/token", {
                    method: "POST",
                    headers: { "content-type": "application/x-www-form-urlencoded" },
                    body: new URLSearchParams({
                      client_id: process.env.GOOGLE_CLIENT_ID!,
                      code: code!,
                      redirect_uri: process.env.APP_URL! + "/api/auth/callback",
                      grant_type: "authorization_code",
                    }),
                  });

                  return Response.json({ ok: tokenResponse.ok });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("oauth-callback-pkce:")));
}

#[test]
fn skips_oauth_pkce_issue_when_callback_sends_code_verifier() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/auth/callback/route.ts",
        r#"
                import { cookies } from "next/headers";

                export async function GET(request: Request) {
                  const { searchParams } = new URL(request.url);
                  const code = searchParams.get("code");
                  const state = searchParams.get("state");
                  const expectedState = cookies().get("oauth_state")?.value;
                  const codeVerifier = cookies().get("pkce_code_verifier")?.value;

                  if (!state || state !== expectedState || !codeVerifier) {
                    return new Response("invalid callback", { status: 400 });
                  }

                  const tokenResponse = await fetch("https://oauth2.googleapis.com/token", {
                    method: "POST",
                    headers: { "content-type": "application/x-www-form-urlencoded" },
                    body: new URLSearchParams({
                      client_id: process.env.GOOGLE_CLIENT_ID!,
                      code: code!,
                      code_verifier: codeVerifier,
                      redirect_uri: process.env.APP_URL! + "/api/auth/callback",
                      grant_type: "authorization_code",
                    }),
                  });

                  return Response.json({ ok: tokenResponse.ok });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("oauth-callback-pkce:")));
}
