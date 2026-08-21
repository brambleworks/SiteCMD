import type { CodeFixGuideEntry } from "./types";

export const AI_FIX_GUIDES: Record<string, CodeFixGuideEntry> = {
  "ai-cache-dedupe": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Start with in-flight request deduplication so simultaneous identical calls within the same authorization scope share one provider promise, and clear the in-flight map on success, failure, cancellation, and timeout.",
      "Cache responses only when the key includes tenant/user scope, the complete effective request, model and parameters, and data versions, and never cache personalized, permission-dependent, or sensitive output without an explicit isolation, retention, and deletion design. Test that two tenants with identical prompts can never receive each other's result.",
    ],
  },
  "ai-concurrency": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Bound local fan-out with a semaphore such as `p-limit`, choosing the limit from provider quotas, latency, memory, and cost rather than a universal number, and release permits in a finally path with cancellation propagated. A module-level limiter only caps one process; multi-instance or serverless deployments need a shared or durable mechanism plus per-tenant fairness for a true fleet-wide cap.",
    ],
  },
  "ai-kill-switch-missing": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Inventory existing controls at the AI provider, API gateway, deploy platform, and feature-flag layer first; the finding may reflect a control the scanner could not see. Confirm who can use each one, what provider calls it covers, and how quickly it propagates.",
      "If a gap remains, enforce the disable decision at the trusted server-side boundary before creating any provider request or job, scoped to the feature, provider, tenant, or product as the incident model requires. Use an environment setting only when restart or redeploy latency meets the response objective; otherwise use an authenticated, authorized, audited remote control with a deploy-time fallback, and define what happens to in-flight work.",
    ],
  },
  "ai-loop-risk": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Trace the flagged loop and confirm the provider call is actually inside it; imported helpers may already enforce termination. If the flow is effectively unbounded, add feature-derived limits at the trusted server boundary, such as maximum iterations or elapsed time, cumulative token and spend budgets, and cancellation, and prevent one tick or job from overlapping the next unless concurrency is intentional.",
    ],
  },
  "ai-observability": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Check first whether the provider SDK, a shared wrapper, gateway, or hosting platform already instruments these calls; the scanner only assessed the flagged file. Where instrumentation is genuinely missing, emit one canonical completion/failure event per call with privacy-reviewed metrics such as model id, latency, outcome class, retry count, and token usage, and never record prompts, responses, credentials, or raw user identifiers by default.",
    ],
  },
  "ai-observability-integration-missing": {
    effort: "moderate",
    effortMinutes: 25,
    default: [
      "Choose an observability backend that fits the product's privacy, retention, hosting, and cost requirements, initialize its server SDK at the application boundary, and wrap provider calls in spans and metrics covering the model identifier, token counts when the provider supplies them, latency, retry count, and error class without recording prompts, responses, or credentials. Set alerts from the feature's own baseline and SLO rather than a universal percentage.",
    ],
  },
  "ai-output-cap": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Set the server-side output limit for the API you actually call; the field differs by provider (`max_output_tokens` for OpenAI Responses, `max_completion_tokens` for current Chat Completions models, `max_tokens` for Anthropic Messages), so confirm the installed SDK rather than copying one name. Choose the ceiling from the expected answer shape and budget, and surface truncation to users instead of silently returning malformed output.",
    ],
  },
  "ai-rate-limit": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Rate-limit every path that can start provider work, including streaming and background jobs, with limits derived from the feature's latency and spend budget rather than one universal requests-per-minute number. Key authenticated limits by user plus tenant or billing account, add a separate pre-auth control for anonymous entry points, use a shared atomic store across instances, and return 429 with a valid `Retry-After` so rejected requests never reach the provider.",
    ],
  },
  "ai-request-validation": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Validate user input before it enters a prompt: enforce type, length, and allowed-format constraints with a schema library such as Zod or Joi (for example `z.string().max(2000).trim()`). Do not treat regex filtering as a fix for prompt injection; keep untrusted content out of system and developer instructions and place it in a clearly delimited user-content section.",
    ],
  },
  "ai-retry-bounds": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Inventory retries the provider SDK, gateway, and calling layers already perform before adding another; an explicit zero-retry policy is valid. Otherwise set a small attempt cap and an overall deadline from the caller's latency and cost budget, retry only provider-documented transient failures with exponential backoff and jitter, honor a valid `Retry-After`, and do not retry validation, authentication, billing, cancellation, or content-policy failures.",
    ],
  },
  "ai-spend-guardrails": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Inventory provider project limits, billing alerts, gateway policy, and existing application accounting first, and distinguish hard pre-call limits from delayed alerts; an alert is not a spending cap. Where a gap remains, reserve an estimated allowance before work starts at the right account, tenant, user, or feature scope, reconcile it against provider-reported usage afterward, and set warning and stop thresholds from the product's budget rather than fixed percentages.",
    ],
  },
  "ai-timeout": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Set bounded connect, response-start, stream-idle, and overall deadlines where the client supports them; a single request timeout can still leave a stalled stream running. Derive each limit from the feature's latency budget, keep the overall deadline below the caller or platform timeout, and propagate an abort signal through retries and streaming so provider work is cancelled and a controlled result is returned.",
    ],
  },
  "ai-user-controlled-model": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Trace the client-supplied value into the effective provider call first; an upstream gateway or server wrapper may already map it to approved models, and that boundary should be documented and tested rather than duplicated. Where client choice is intentional, expose product-level options such as `fast` or `highQuality` and map them server-side to approved model ids, rejecting unknown, deprecated, or unauthorized values before any provider work starts.",
    ],
  },
  "ai-user-controlled-settings": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Keep the authoritative model and generation-policy bounds on the server and never forward an arbitrary client request object into the provider SDK. When user control is intentional, map named presets or validate numeric values against model-specific ranges and product caps, rejecting NaN, infinity, negative limits, unknown fields, and unsupported combinations so the provider receives only bounded allowlisted fields.",
    ],
  },
  "ai-conversation-artifacts": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Remove comments containing conversational phrases like 'As an AI' or 'I\\'ll help you', and rewrite any genuinely useful context as a normal code comment explaining the why. Review the surrounding code too: the artifact shows non-code material reached the source, but this signal alone does not establish that the implementation contains a bug.",
    ],
  },
};
