use super::super::super::*;
use crate::checks::IssueConfidence;

#[test]
fn detects_unbounded_ai_route() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import OpenAI from "openai";
                export async function POST(request: Request) {
                  const body = await request.json();
                  const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
                  return Response.json(await client.responses.create({ model: "gpt-4.1", input: body.message }));
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    for slug in [
        "ai-rate-limit",
        "ai-timeout",
        "ai-concurrency",
        "ai-spend-guardrails",
    ] {
        let issue = report
            .issues
            .iter()
            .find(|issue| issue.id.starts_with(&format!("{slug}:")))
            .unwrap_or_else(|| panic!("expected {slug} finding"));
        assert_eq!(
            issue.severity,
            Severity::Medium,
            "wrong severity for {slug}"
        );
        assert_eq!(
            issue.confidence,
            IssueConfidence::NeedsReview,
            "wrong confidence for {slug}"
        );
        assert!(
            issue.title.starts_with("No recognized"),
            "title overstates detector visibility for {slug}: {}",
            issue.title
        );
        assert!(
            issue.description.contains("scanned file"),
            "description omits scan scope for {slug}: {}",
            issue.description
        );
    }
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ai-request-validation:")));
}

#[test]
fn detects_webhook_without_signature_verification() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/webhook/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.text();
                  console.log(body);
                  return new Response("ok");
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("webhook-signature:"))
        .expect("webhook-signature issue"); // allow-expect: test assertion
    assert!(issue.title.contains("No recognized"));
    assert!(issue.description.contains("scanned file"));
    assert!(issue.description.contains("does not establish"));
    assert!(issue
        .verify_hint
        .as_deref()
        .unwrap_or_default()
        .contains("test fixture"));
}

#[test]
fn does_not_flag_webhook_named_dto_or_service_files() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "apps/api/v2/src/modules/webhooks/outputs/webhook.output.ts",
        r#"
                import { IsInt, IsString } from "class-validator";
                export class WebhookOutputDto {
                  @IsInt() readonly id!: number;
                  @IsString() readonly subscriberUrl!: string;
                }
            "#,
    );
    write_file(
        temp.path(),
        "apps/api/v2/src/modules/webhooks/services/webhooks.service.ts",
        r#"
                export class WebhooksService {
                  async findWebhook(id: number) {
                    return this.repository.findById(id);
                  }
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("webhook-signature:")),
        "DTO/service files under a webhooks/ dir must not be flagged as webhook handlers: {:?}",
        report
            .issues
            .iter()
            .map(|i| &i.id)
            .filter(|id| id.starts_with("webhook-signature:"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn suppresses_webhook_verified_via_construct_webhook_event() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "pages/api/integrations/vital/webhook.ts",
        r#"
                export default async function handler(req, res) {
                  const sig = req.headers["svix-signature"];
                  if (!sig) return res.status(400).end();
                  const payload = JSON.stringify(req.body);
                  const event = vitalClient.Webhooks.constructWebhookEvent(payload, req.headers, process.env.VITAL_WEBHOOK_SECRET);
                  return res.status(200).json({ ok: true, type: event.event_type });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("webhook-signature:")),
        "a webhook verified via constructWebhookEvent must not be flagged"
    );
}

#[test]
fn detects_webhook_without_idempotency_guard() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/webhook/route.ts",
        r#"
                import Stripe from "stripe";

                export async function POST(request: Request) {
                  const payload = await request.text();
                  const event = stripe.webhooks.constructEvent(payload, request.headers.get("stripe-signature"), process.env.STRIPE_WEBHOOK_SECRET);
                  await db.orders.update({ where: { id: event.data.object.id }, data: { status: "paid" } });
                  return new Response("ok");
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("webhook-idempotency:"))
        .expect("webhook-idempotency issue"); // allow-expect: test assertion
    assert!(issue.title.contains("No recognized"));
    assert!(issue.description.contains("does not establish"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("atomic"));
}
