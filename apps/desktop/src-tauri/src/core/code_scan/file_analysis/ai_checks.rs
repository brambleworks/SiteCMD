use super::*;

pub(super) fn collect_ai_issues(issues: &mut Vec<CodeIssue>, ctx: &FileAnalysisContext<'_>) {
    let file = ctx.file;
    let content = ctx.content;
    let route_like = ctx.signals.route_like;
    let has_validation = ctx.signals.has_validation;
    let has_rate_limit = ctx.signals.has_rate_limit;
    let has_timeout = ctx.signals.has_timeout;
    let has_concurrency_guard = ctx.signals.has_concurrency_guard;
    let uses_llm = ctx.signals.uses_llm;
    let uses_ai_sdk = ctx.signals.uses_ai_sdk;
    let has_llm_spend_guard = ctx.signals.has_llm_spend_guard;
    let has_llm_usage_logging = ctx.signals.has_llm_usage_logging;
    let has_llm_observability = ctx.signals.has_llm_observability;
    let has_llm_output_cap = ctx.signals.has_llm_output_cap;
    let has_llm_cache_dedupe = ctx.signals.has_llm_cache_dedupe;
    let has_user_controlled_llm_model = ctx.signals.has_user_controlled_llm_model;
    let has_user_controlled_llm_settings = ctx.signals.has_user_controlled_llm_settings;
    let has_model_allowlist = ctx.signals.has_model_allowlist;
    let has_numeric_bounds = ctx.signals.has_numeric_bounds;
    let has_loop_pattern = ctx.signals.has_loop_pattern;
    let has_loop_guard = ctx.signals.has_loop_guard;
    let parses_body = ctx.signals.parses_body;
    let likely_public_endpoint = ctx.signals.likely_public_endpoint;

    if route_like && uses_llm && !has_timeout {
        issues.push(build_issue(
            "ai-timeout",
            "ai-safety",
            Severity::High,
            "No recognized timeout or cancellation for AI route",
            "This route-like file contains recognized AI provider usage, but SiteCMD found no recognized timeout, abort signal, or cancellation pattern in the scanned file. An imported wrapper, configured client, gateway, platform deadline, or job policy may still bound the effective call. If no appropriate bound exists, stalled or long-running calls can consume capacity, delay responses, and increase retry-related cost.",
            file,
            first_llm_usage_line(content),
            Some("A recognized AI provider call was detected, but no local timeout, abort-signal, or cancellation pattern was found; imported and deployment-level controls were not resolved.".into()),
            Some("Trace the effective client, wrapper, retry, streaming, and platform deadlines first. If they do not provide an appropriate bound, add server-side connect, response-start, stream-idle, and overall cancellation behavior at the layer that owns the provider call.".into()),
            Some("Simulate connection, first-token, and mid-stream stalls plus caller cancellation. Confirm the effective deadline stops local work and, where the provider supports it, cancels the upstream request without an unbounded retry chain.".into()),
        ));
    }

    if let Some(client_directive_line) = client_component_directive_line(content) {
        if client_makes_direct_ai_call(ctx.signals.lower.as_str()) {
            issues.push(build_issue(
                "client-ai-sdk",
                "ai-safety",
                Severity::High,
                "Direct AI provider call appears in client-side code",
                "This client component appears to instantiate an AI provider client or call a model directly. That may expose a long-lived provider credential or bypass server-side usage controls. A provider-documented ephemeral browser credential can be legitimate, so verify the credential lifetime and allowed capabilities before changing the architecture.",
                file,
                Some(client_directive_line),
                Some("A client component directive and a direct provider construction or model-call pattern were detected in the same file; type-only imports and client chat hooks do not trigger this rule.".into()),
                Some("Move long-lived provider credentials and model calls behind a server route or server action. If the provider explicitly supports browser use, mint a short-lived, least-privilege ephemeral credential on the server and document its restrictions.".into()),
                Some("If you moved the call server-side, build the app and confirm the browser bundle no longer contains the provider call or a long-lived credential path. If you intentionally retain a provider-supported ephemeral flow, inspect the built bundle and a real browser request to confirm the token is minted server-side, short-lived, least-privilege, and unable to use unapproved models or spend beyond its limits.".into()),
            ));
        }
    }

    // React PDF style props are document layout, not browser inline CSS, and
    // next/og renders through Satori, which supports inline styles only.
    let react_pdf_renderer =
        content.contains("@react-pdf/renderer") || content.contains("react-pdf-browser");
    // The imports and the metadata-route basenames are precise; a bare
    // `ImageResponse` identifier is not, so it is deliberately not a needle.
    let og_image_renderer = content.contains("next/og")
        || content.contains("@vercel/og")
        || is_og_image_route(&file.relative_path);
    if is_jsx_or_tsx_file(&file.relative_path) && !react_pdf_renderer && !og_image_renderer {
        let static_styles = static_inline_style_offsets(content);
        let inline_style_count = static_styles.len();
        if inline_style_count >= 4 {
            let line = static_styles
                .first()
                .map(|offset| line_number(content, *offset));
            issues.push(build_issue(
                "jsx-inline-style-density",
                "architecture",
                Severity::Low,
                "Review repeated JSX inline styles",
                "This file contains several JSX style props. Inline styles are not inherently incorrect, and runtime-derived values often belong there. Repeated static declarations can still make shared visual rules harder to reuse, override, and keep consistent, so review whether these instances represent one reusable pattern.",
                file,
                line,
                Some(format!(
                    "Detected {} JSX inline style props (`style={{{{ ... }}}}`) with literal values in the same component file.",
                    inline_style_count
                )),
                Some("Keep genuinely dynamic values inline. If the detected declarations repeat one static visual pattern, move that pattern into the project's existing styling system or a small shared component; leave unrelated one-off declarations alone.".into()),
                Some("Use visual or screenshot regression coverage across the affected states and responsive sizes. Re-run Code Scan only after the justified repeated static styles have been reduced; a clean scan is not a reason to replace necessary dynamic styles.".into()),
            ));
        }
    }

    if route_like && uses_llm && !has_rate_limit {
        issues.push(build_issue(
            "ai-rate-limit",
            "ai-safety",
            // Emit the effective High severity applied by policy.
            Severity::High,
            "No recognized application rate limit for AI route",
            "This route-like file contains recognized AI provider usage, but SiteCMD found no local rate-limit or quota pattern in the scanned file. Shared middleware, an API gateway, a provider limit, or a wrapper may still enforce one. If the effective request path has no caller-aware limit, abusive or accidental request bursts can consume quota, capacity, and budget.",
            file,
            first_llm_usage_line(content),
            Some("A recognized AI provider call was detected, but no local limiter pattern or inherited middleware signal was found; gateway and provider controls were not resolved.".into()),
            Some("Trace every entry point that can start provider work and inventory effective gateway, middleware, account, tenant, user, network, token, concurrency, and spend limits. Add the missing control at the deployment scope that must share state, before reserving or starting provider work.".into()),
            Some("Load-test bursts and sustained traffic across users, tenants, and multiple application instances. Confirm rejected requests do not reach the provider, allowed traffic remains fair, and HTTP limits return the intended 429 and Retry-After behavior.".into()),
        ));
    }

    if route_like && uses_llm && !has_concurrency_guard {
        issues.push(build_issue(
            "ai-concurrency",
            "ai-safety",
            Severity::High,
            "No recognized concurrency control for AI route",
            "This route-like file contains recognized AI provider usage, but SiteCMD found no local queue, semaphore, or concurrency-cap pattern in the scanned file. An imported worker, shared queue, platform setting, or provider quota may still bound in-flight work. If no effective bound exists, overlapping requests can exhaust connections or memory and multiply spend before a request-rate limit reacts.",
            file,
            first_llm_usage_line(content),
            Some("A recognized AI provider call was detected, but no local p-limit, Bottleneck, semaphore, queue, or maxConcurrency pattern was found; imported and deployment-level controls were not resolved.".into()),
            Some("Trace where provider work is actually scheduled. If the effective path is unbounded, add a cancellation-safe concurrency limit at the required process, tenant, region, or fleet scope, with bounded queue wait and fair admission.".into()),
            Some("Run overlapping requests across the deployed instance topology. Measure the maximum in-flight provider calls, queue wait, cancellation, overload response, and recovery after failures.".into()),
        ));
    }

    if route_like && uses_llm && !has_llm_spend_guard && !has_llm_usage_logging {
        issues.push(build_issue(
            "ai-spend-guardrails",
            "ai-safety",
            Severity::High,
            "No recognized spend guard or usage accounting for AI route",
            "This route-like file contains recognized AI provider usage, but SiteCMD found no local budget, quota, token-accounting, or usage-recording pattern in the scanned file. Provider projects, gateways, billing systems, or imported wrappers may supply these controls. If the effective path has neither pre-call limits nor post-call accounting, cost growth and retry loops are harder to contain or detect.",
            file,
            first_llm_usage_line(content),
            Some("A recognized AI provider call was detected, but no local budget, quota, token-usage, or cost-accounting pattern was found; provider and external billing controls were not resolved.".into()),
            Some("Inventory provider- and gateway-level budgets first. If a gap remains, enforce an estimated pre-call allowance at the account or tenant boundary, reconcile it against provider-reported usage after completion, and alert on feature-specific spend and anomaly thresholds.".into()),
            Some("Exercise normal, maximum-length, retry, cancellation, and concurrent calls. Confirm the allowance is checked before work starts, actual usage is reconciled afterward, failures release or settle reservations correctly, and alerts fire without recording prompt content.".into()),
        ));
    }

    if route_like && uses_llm && (uses_ai_sdk || has_llm_spend_guard) && !has_llm_observability {
        issues.push(build_issue(
            "ai-observability",
            "ai-safety",
            Severity::Medium,
            "No recognized provider-aware telemetry for AI route",
            "This route-like file contains recognized AI provider usage, but SiteCMD found no local combination of usage telemetry and correlation or provider context in the scanned file. An imported wrapper, SDK instrumentation, gateway, or runtime agent may still emit it. Effective telemetry should identify the approved provider/model, latency, outcome, retry and token usage when available, using a safe correlation id without recording prompts or responses by default.",
            file,
            first_llm_usage_line(content),
            Some("A recognized AI provider call was detected, but the scanned file did not contain both usage telemetry and a recognized correlation, finish-hook, or provider-context pattern; inherited instrumentation was not resolved.".into()),
            Some("Trace existing SDK, wrapper, gateway, and runtime instrumentation. If a gap remains, emit privacy-reviewed metrics and a structured completion/failure event with a non-secret correlation id, approved provider/model id, latency phases, outcome, retry count, token usage when supplied, and estimated cost where the estimate is defined.".into()),
            Some("Exercise success, provider rejection, timeout, retry, cancellation, and partial-stream paths. Confirm metrics and traces correlate without storing prompts, responses, credentials, raw user identifiers, or sensitive retrieval data.".into()),
        ));
    }

    if route_like && uses_llm && has_user_controlled_llm_model && !has_model_allowlist {
        issues.push(build_issue(
            "ai-user-controlled-model",
            "ai-safety",
            Severity::High,
            "Request-derived model selector lacks a recognized local allowlist",
            "SiteCMD matched a request-derived model or provider selector near recognized AI usage, but found no recognized allowlist or server-owned model map in the scanned file. Imported validation, an upstream policy, or a wrapper may still constrain the value. If the raw selector reaches the provider, callers may choose unsupported, more expensive, or policy-inappropriate models.",
            file,
            first_match_line(content, &USER_CONTROLLED_LLM_MODEL_PATTERNS)
                .or_else(|| first_llm_usage_line(content)),
            Some("A request-derived model/provider expression was detected, but no local z.enum, allowlist, model map, switch, or explicit supported-model check was found; imported validation and wrapper policy were not resolved.".into()),
            Some("Trace the selector into the effective provider request. If it is not already constrained, map a small set of product-facing choices to server-owned approved model ids and enforce entitlement, capability, region, and spend policy before provider work starts.".into()),
            Some("Send each approved choice plus unknown, deprecated, unauthorized, and higher-cost model ids. Confirm only the server mapping reaches the provider and rejected choices start no billable work.".into()),
        ));
    }

    if route_like && uses_llm && !has_llm_output_cap {
        issues.push(build_issue(
            "ai-output-cap",
            "ai-safety",
            Severity::Medium,
            "No recognized AI output cap was found in this route",
            "This route contains a recognized AI provider call, but SiteCMD found no recognized token or output limit in the scanned file. This does not prove the effective call is unbounded: an imported wrapper, configured client, gateway, provider requirement, or provider default may impose a ceiling. If none does, output size, latency, and spend can exceed the feature's intended budget.",
            file,
            first_llm_usage_line(content),
            Some("A recognized provider call was detected, but no max_tokens, max_completion_tokens, max_output_tokens, or equivalent pattern was found in the scanned file; wrapper and provider configuration were not resolved.".into()),
            Some("Trace the effective provider request first. If no appropriate ceiling exists, set the API-specific server-side output limit at the trusted model boundary and choose it from the feature's response shape, latency, and cost budget. Keep any provider default only when it is documented and tested.".into()),
            Some("Capture the effective provider request in a test or mock, confirm the intended output field and value reach the provider, then exercise maximum-length, truncation/stop-reason, streaming, structured-output, storage, and UI behavior.".into()),
        ));
    }

    if route_like
        && uses_llm
        && has_user_controlled_llm_settings
        && !(has_validation && has_numeric_bounds)
    {
        let controls_output_limit = has_any(content, &USER_CONTROLLED_LLM_OUTPUT_LIMIT_PATTERNS);
        let (severity, title, impact) = if controls_output_limit {
            (
                Severity::High,
                "Request-derived AI output limit lacks recognized local bounds",
                "If an unbounded request value reaches the provider, callers can expand response size, latency, and spend up to the provider's own limit.",
            )
        } else {
            (
                Severity::Medium,
                "Request-derived AI generation setting lacks recognized local bounds",
                "If an unbounded request value reaches the provider, callers can select unsupported or product-inappropriate sampling behavior even when the provider enforces its own numeric range.",
            )
        };
        issues.push(build_issue(
            "ai-user-controlled-settings",
            "ai-safety",
            severity,
            title,
            &format!("SiteCMD matched a request-derived AI output or generation setting near recognized provider usage, but found no recognized bounded schema or clamp in the scanned file. Imported validation, normalization, or a wrapper may still constrain the value. {impact}"),
            file,
            first_match_line(content, &USER_CONTROLLED_LLM_SETTING_PATTERNS)
                .or_else(|| first_llm_usage_line(content)),
            Some("A request-derived AI setting expression was detected, but no local clamp, min/max schema bound, or recognized normalization path was found; imported validation and wrapper policy were not resolved.".into()),
            Some("Trace the value into the effective provider request. If the product intentionally exposes it and no effective bound exists, allowlist the field and enforce finite, model-specific server bounds; otherwise keep the setting server-owned and ignore or reject the client field.".into()),
            Some("Test the intended minimum and maximum, just-outside values, malformed numbers, unsupported combinations, and omitted fields. Confirm the provider receives only finite allowlisted values within the server-owned product budget.".into()),
        ));
    }

    if likely_public_endpoint && uses_llm && parses_body && !has_llm_cache_dedupe {
        issues.push(build_issue(
            "ai-cache-dedupe",
            "ai-safety",
            Severity::Low,
            "Review duplicate-request handling for this AI route",
            "This route appears able to accept request input and start AI work, but SiteCMD found no recognized in-flight dedupe, idempotency, or cache pattern in the scanned file. Such reuse is not universally appropriate: repeated prompts may intentionally produce different results, and shared caching can cross authorization or privacy boundaries. It is useful when retries or duplicate submissions are meant to represent the same logical operation.",
            file,
            first_llm_usage_line(content),
            Some("The route parses request input for recognized AI usage, but no local in-flight dedupe, idempotency-key, or cache pattern was found; client, gateway, queue, and wrapper behavior were not resolved.".into()),
            Some("Trace client retries, refreshes, queue redelivery, and double-submit behavior. Add scoped in-flight dedupe or idempotency only when repeated requests should share work; cache completed output only with an explicit authorization, privacy, retention, invalidation, and non-determinism design.".into()),
            Some("Reproduce the actual duplicate path and confirm one logical operation starts at most the intended provider work. Also prove distinct requests and different users or tenants never share state unless that sharing is explicitly authorized.".into()),
        ));
    }

    if uses_llm && has_loop_pattern && !has_loop_guard {
        issues.push(build_issue(
            "ai-loop-risk",
            "ai-safety",
            // Emit the effective High severity applied by policy.
            Severity::High,
            "Possible unbounded AI loop needs review",
            "Recognized AI provider usage and an unbounded loop or interval primitive occur in the same scanned file, with no recognized local attempt cap or cancellation pattern. This co-occurrence does not prove the provider call executes inside that loop or that an imported control is absent. If it does execute without a hard stop, a failure or overlap can repeat billable work indefinitely.",
            file,
            first_match_line(content, &LOOP_PATTERNS).or_else(|| first_llm_usage_line(content)),
            Some("Recognized AI usage and a while(true), for(;;), or setInterval pattern were detected in the same file without a recognized local max-attempt, clearInterval, or cancellation guard; value flow and imported controls were not resolved.".into()),
            Some("Trace the actual control flow first. If provider work is inside an intentionally repeating process, add a feature-specific stop condition, total time/token/spend budget, overlap prevention, cancellation propagation, and failure backoff rather than relying on a magic iteration count.".into()),
            Some("Exercise success, persistent failure, timeout, cancellation, restart, and overlapping ticks. Confirm provider work terminates or pauses at the defined budget and that a previous iteration cannot silently overlap the next.".into()),
        ));
    }
}

/// Detect direct provider calls in lowercased client code.
/// Vercel AI SDK React hooks and transports are excluded because they call a
/// server route without holding provider credentials.
fn client_makes_direct_ai_call(lower: &str) -> bool {
    const DIRECT_CALL_NEEDLES: &[&str] = &[
        "new openai(",
        "new anthropic(",
        "streamtext(",
        "generatetext(",
        "generateobject(",
        "streamobject(",
        "chat.completions.create",
        "embeddings.create",
        "generatecontent(",
        "api.openai.com",
        "api.anthropic.com",
    ];
    DIRECT_CALL_NEEDLES
        .iter()
        .any(|needle| lower.contains(needle))
}

/// Next.js metadata image routes render through `ImageResponse`.
fn is_og_image_route(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    base.starts_with("opengraph-image.") || base.starts_with("twitter-image.")
}

/// Byte offsets of `style={{` props whose object carries a literal value. An
/// object that does not close within the scan cap is counted rather than
/// dropped, so an unread tail cannot silently shrink the density.
fn static_inline_style_offsets(content: &str) -> Vec<usize> {
    JSX_INLINE_STYLE_PROP_PATTERN
        .find_iter(content)
        .filter(|found| match style_object_window(content, found.end()) {
            Some(window) => JSX_STATIC_STYLE_VALUE_PATTERN.is_match(window),
            None => true,
        })
        .map(|found| found.start())
        .collect()
}

/// The style object's text up to its closing brace. `None` when the object does
/// not close within the scan cap, so the caller can fail safe instead of
/// grading a partially read object.
fn style_object_window(content: &str, after: usize) -> Option<&str> {
    const CAP: usize = 600;
    let bytes = content.as_bytes();
    let mut depth = 1usize;
    let mut end = after;
    let cap = (after + CAP).min(content.len());
    while end < cap {
        match bytes[end] {
            b'{' => depth += 1,
            // An ASCII brace is always a character boundary, so the slice below
            // is safe without a boundary walk.
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&content[after..end]);
                }
            }
            _ => {}
        }
        end += 1;
    }
    None
}

#[cfg(test)]
mod inline_style_tests {
    use super::{static_inline_style_offsets, style_object_window};

    #[test]
    fn only_literal_valued_style_objects_are_counted() {
        let runtime = "<span style={{ color: team.primaryColor }} />";
        assert!(static_inline_style_offsets(runtime).is_empty());

        let literal = "<span style={{ color: \"var(--team-primary, #2563eb)\" }} />";
        assert_eq!(static_inline_style_offsets(literal).len(), 1);

        // A nested object closes at its own brace, not the first one seen.
        let nested = "<span style={{ transform: { scale: sizes.large } }} />";
        assert!(static_inline_style_offsets(nested).is_empty());
    }

    #[test]
    fn a_style_object_longer_than_the_scan_cap_is_counted_not_dropped() {
        let filler = "runtimeValue, ".repeat(70);
        let oversized = format!("<span style={{{{ a: {}b: last }}}} />", filler);
        assert!(
            style_object_window(&oversized, oversized.find("{{").unwrap() + 2).is_none(),
            "the fixture must exceed the scan cap for this test to mean anything"
        );
        assert_eq!(
            static_inline_style_offsets(&oversized).len(),
            1,
            "an object the scanner could not finish reading must not be graded as runtime-only"
        );
    }
}

#[cfg(test)]
mod client_ai_tests {
    use super::client_makes_direct_ai_call;

    #[test]
    fn ignores_vercel_ai_sdk_react_hooks() {
        // The common, safe modern pattern: a chat UI client component using the
        // AI SDK React hooks + types. Must NOT be flagged.
        let chat_ui = r#"
            import { useChat } from "@ai-sdk/react";
            import { DefaultChatTransport, type UIMessage } from "ai";
            const { messages, sendMessage } = useChat({ transport: new DefaultChatTransport() });
        "#
        .to_lowercase();
        assert!(!client_makes_direct_ai_call(&chat_ui));
    }

    #[test]
    fn flags_direct_provider_calls_in_client_code() {
        let leaky = r#"import OpenAI from "openai";
            const openai = new OpenAI({ apiKey: process.env.NEXT_PUBLIC_OPENAI_KEY });
            const res = await openai.chat.completions.create({ model: "gpt-4" });"#
            .to_lowercase();
        assert!(client_makes_direct_ai_call(&leaky));
        assert!(client_makes_direct_ai_call(
            &"await streamText({ model });".to_lowercase()
        ));
    }
}
