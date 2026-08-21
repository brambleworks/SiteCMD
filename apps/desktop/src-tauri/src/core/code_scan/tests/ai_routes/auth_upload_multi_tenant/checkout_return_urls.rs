use super::super::super::*;

#[test]
fn detects_stripe_checkout_with_user_controlled_return_urls() {
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
                    mode: "payment",
                    line_items: [{ price: "price_fixed", quantity: 1 }],
                    success_url: body.successUrl,
                    cancel_url: body.cancelUrl,
                  });

                  return Response.json({ id: session.id });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("stripe-user-controlled-redirect:")));
}

#[test]
fn skips_stripe_checkout_redirect_issue_when_return_url_is_guarded() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/billing/route.ts",
        r#"
                import { z } from "zod";
                import Stripe from "stripe";

                const stripe = new Stripe(process.env.STRIPE_SECRET!, { apiVersion: "2024-06-20" });

                export async function POST(request: Request) {
                  const body = z.object({
                    redirectTo: z.string().min(1),
                  }).parse(await request.json());

                  if (!body.redirectTo.startsWith("/")) {
                    throw new Error("invalid redirect");
                  }

                  const target = new URL(body.redirectTo, process.env.APP_URL);
                  if (target.origin !== process.env.APP_URL) {
                    throw new Error("invalid origin");
                  }

                  const session = await stripe.checkout.sessions.create({
                    mode: "payment",
                    line_items: [{ price: "price_fixed", quantity: 1 }],
                    success_url: target.toString(),
                    cancel_url: new URL("/billing", process.env.APP_URL).toString(),
                  });

                  return Response.json({ id: session.id });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("stripe-user-controlled-redirect:")));
}
