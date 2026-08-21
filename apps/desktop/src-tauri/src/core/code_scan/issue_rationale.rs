//! Rule-specific impact rationale for findings built through `build_issue`.

pub(super) fn code_issue_rationale(slug: &str) -> Option<&'static str> {
    Some(match slug {
        // AI safety and cost controls.
        "ai-timeout" => "A stalled provider call can hold request resources after the user has stopped waiting and may continue consuming billable work until the provider or runtime terminates it.",
        "client-ai-sdk" => "A direct browser call can expose a long-lived provider credential or bypass server-side authorization, usage limits, and audit controls unless it uses a deliberately constrained ephemeral-token flow.",
        "ai-rate-limit" => "Without a caller-specific limit, one user or automated client can consume shared provider quota and spend, degrading the feature for everyone else.",
        "ai-spend-guardrails" => "Rate and concurrency limits bound traffic shape, but a separate usage budget is what limits cumulative cost per user, tenant, and billing period.",
        "ai-user-controlled-model" => "Accepting raw model identifiers lets a caller select unapproved, unavailable, or unexpectedly expensive models outside the product's tested policy.",
        "ai-user-controlled-settings" => "If the matched request-derived value reaches the provider without an effective product bound, output limits can increase latency and spend while sampling controls can select behavior outside the feature's tested policy.",
        "ai-concurrency" => "Parallel model calls can multiply provider quota use, memory pressure, and spend even when each individual request is rate-limited.",
        "ai-output-cap" => "When neither the provider, client wrapper, nor gateway supplies an appropriate ceiling, a provider-side output limit bounds worst-case generation latency and spend; truncating only after receipt does not recover tokens already generated.",
        "ai-loop-risk" => "If the matched provider call executes inside the unbounded loop or interval without an effective stop, failure or overlap can repeat billable work until an external limit intervenes.",
        "ai-retry-bounds" => "Zero retries can be correct; when retries are enabled by the application, SDK, gateway, or queue, explicit attempt and time budgets prevent an outage from multiplying billable work or repeating side effects.",
        "ai-observability" => "When no wrapper, gateway, SDK, or runtime layer supplies equivalent telemetry, missing latency, outcome, model, and usage signals makes provider failures and cost regressions harder to attribute reliably.",
        "ai-cache-dedupe" => "When retries or duplicate submissions represent one logical operation, they can repeat provider work and cost; deduplication is optional and must preserve authorization, privacy, tenant isolation, and intended non-determinism.",
        "ai-kill-switch-missing" => "When no equivalent provider, gateway, or platform control exists, an operational disable path lets authorized operators stop new provider work during a cost spike, abuse event, or provider incident on a defined propagation timeline.",
        "ai-conversation-artifacts" => "Conversational residue is not a runtime vulnerability, but it is concrete evidence that generated code may have been copied without a focused review of surrounding assumptions.",
        "ai-request-validation" => "Schema and size validation bound malformed or oversized AI requests before they consume provider quota; it does not replace prompt-injection-resistant feature design.",

        // Authentication, authorization, session, and request integrity.
        "client-auth-without-server-enforcement" => "Client-side auth signals do not establish which routes are protected, but every protected server boundary must independently enforce authentication and authorization because a caller can bypass navigation and invoke it directly.",
        "localstorage-auth-token" => "A token readable by page JavaScript is available to any successful same-origin script injection, so its storage and lifetime should match the application's threat model.",
        "jwt-decode-without-verify" => "Decoding exposes claims without proving who signed them; using unverified claims for access decisions lets an attacker supply arbitrary identity or role data.",
        "session-cookie-flags" => "Cookie flags limit browser delivery and script access. Missing flags can widen exposure to network interception, script injection, or cross-site requests depending on which attribute is absent.",
        "sensitive-auth" => "If no effective upstream gate exists, a sensitive action can be reached without a verified server-side identity; the file-local absence must be reconciled with middleware, proxy, and framework enforcement first.",
        "sensitive-authz" => "Authentication proves identity, not permission. If middleware, services, and database policy do not supply the decision elsewhere, a sensitive action without a role, ownership, tenant, or capability policy may be available to the wrong signed-in account.",
        "csrf-missing" => "When authentication rides automatically on cookies, another site may be able to cause a victim's browser to submit a state-changing request unless the server validates an anti-forgery signal.",
        "oauth-callback-state" => "OAuth state binds the callback to the browser flow that initiated it; without that binding, login CSRF and account-linking confusion become possible.",
        "oauth-callback-pkce" => "PKCE binds an intercepted authorization code to the client instance that began the flow, reducing the value of a stolen code.",
        "one-time-token-no-expiry" => "A redemption token with no enforced expiry remains usable for as long as it exists, extending the window after an email, log, or URL leak.",
        "one-time-token-no-single-use" => "A token that is not consumed atomically can be replayed after its intended action and can race across simultaneous redemption requests.",
        "one-time-token-raw-lookup" => "Storing the bearer value directly turns a database disclosure into immediately usable reset or invitation links; a stored hash limits that direct reuse.",
        "tenant-scope-missing" => "A query can be correctly authenticated and still cross a tenant boundary if ownership or tenant scope is not enforced at the data-access point.",

        // Injection, unsafe interpretation, and outbound navigation.
        "eval-exec-injection" => "A request-derived expression near dynamic evaluation deserves immediate value-flow review, but this shallow same-file match does not by itself prove exploitable runtime reachability.",
        "shell-injection" => "Request parsing and process execution in one file deserve a focused value-flow review, but the co-occurrence alone does not prove that attacker-controlled data reaches a shell or command argument.",
        "js-command-injection" | "python-command-injection"
        | "php-dynamic-command" => "The static match identifies a request accessor at a process boundary; if the value is attacker-controlled on a reachable path, shell grammar, executable selection, or command-specific option parsing may change the intended operation.",
        "raw-sql-unsafe" | "python-sql-injection" => "The static match needs value-flow and validation review; if attacker-controlled text reaches SQL structure, it can alter the query within the connected role's privileges, while bound values remain separate from SQL syntax.",
        "unsafe-html" => "Rendering untrusted HTML can execute script or inject active markup in the application's origin unless an appropriate allowlist sanitizer runs at the final rendering boundary.",
        "user-controlled-fetch" => "A server-side request to an attacker-chosen destination can reach internal services, metadata endpoints, or trusted network locations unavailable to the attacker directly.",
        "open-redirect" | "python-open-redirect" => "An attacker-controlled redirect can make a trusted application URL deliver users to a phishing or credential-capture destination after login or another sensitive flow.",
        "python-code-execution" | "php-code-execution" => "The static match identifies a request accessor inside a language evaluator; if that value is attacker-controlled on a reachable runtime path, evaluated source executes with the application's process privileges.",
        "python-template-injection" => "The static match identifies a request accessor in template source; if an attacker controls it, template expressions can expose context and reach environment-dependent capabilities beyond ordinary variable interpolation.",
        "python-unsafe-deserialization" | "php-object-injection" => "The static match identifies request data at an object-capable deserializer; if integrity and loader restrictions do not intervene, attacker-controlled objects can invoke constructors, magic methods, gadgets, or other type behavior.",
        "python-path-traversal" | "php-path-traversal" | "php-file-inclusion" => "The static match identifies a request accessor in a path or include target; if effective normalization and root constraints do not intervene, traversal can select a file outside the intended scope.",

        // Secrets, transport, debug posture, and cross-origin policy.
        "hardcoded-secret" => "A real credential embedded in source can enter history, artifacts, and logs where access is difficult to revoke selectively; a credential-shaped literal still needs validity and exposure review before that impact is asserted.",
        "client-env-secret" => "A live privileged value emitted through a client-public environment namespace would be available to browser users, but the source reference must be correlated with build output and deployed configuration.",
        "plaintext-password" => "A direct password-shaped database write needs its hooks and storage result verified; if plaintext reaches storage, a database or backup disclosure immediately reveals reusable credentials.",
        "weak-default-credential" => "A shipped default credential is predictable to anyone who can reach the service and often survives into deployed environments when setup steps are skipped.",
        "tls-verification-disabled" => "When the matched client or call executes with verification disabled, it no longer authenticates the destination certificate, so a network-path attacker may be able to impersonate that endpoint.",
        "framework-debug-enabled" => "If the matched configuration is selected by a public runtime, verbose debug handling can expose stack traces, configuration, source paths, query context, or framework tooling.",
        "cors-credentials-wildcard" => "Browsers reject wildcard origins on credentialed CORS responses, so this configuration breaks intended cross-origin access and may conceal middleware behavior that needs explicit review.",
        "cors-origin-reflection" => "Reflecting an arbitrary Origin while allowing credentials can let that origin read a sensitive response when the browser also attaches eligible ambient authentication; route sensitivity, effective configuration, and cookie SameSite behavior determine exploitability.",
        "webhook-signature" => "Without provider-authenticated request verification, anyone who can reach the endpoint may be able to forge events that trigger trusted side effects.",
        "webhook-idempotency" => "Webhook providers retry deliveries and duplicates can arrive concurrently; a unique event claim prevents the same external event from applying side effects more than once.",

        // Payments, uploads, and public endpoints.
        "stripe-user-controlled-price" => "A client-supplied amount or price identifier can bypass the server's product catalog and charge a value the business did not authorize.",
        "stripe-user-controlled-redirect" => "Checkout return URLs influence where customers land after payment and must not turn the trusted payment flow into an open redirect.",
        "stripe-checkout-idempotency" => "Repeated checkout creation can produce duplicate sessions or downstream order work; a stable operation key and server-side order state make retries deterministic.",
        "upload-validation" => "An upload crosses a content and storage boundary. Unchecked type, size, and filename data can enable active content, resource exhaustion, or unsafe downstream processing.",
        "upload-key-scope" => "A client-influenced storage key can overwrite or expose another user's object unless the server derives a unique key inside the authenticated ownership scope.",
        "public-endpoint-rate-limit" => "A public mutation or resource-intensive route can be automated without an account, so a bounded caller and network policy protects capacity and abuse-sensitive workflows.",
        "request-validation" => "Rejecting malformed, oversized, or out-of-range input at the route boundary prevents invalid state and bounds work before business logic and external calls run.",

        // Data correctness and query behavior.
        "client-db-access" => "A server database driver in a client component can expose connection material or move trusted query construction into a browser-controlled environment; browser-designed data APIs are a separate policy model.",
        "db-in-route" => "Embedding persistence logic in route handlers makes transaction, authorization, and query behavior harder to reuse and review consistently across entry points.",
        "multi-write-no-transaction" => "Related writes without an atomic boundary can leave partial state when a later operation fails or two requests interleave.",
        "n-plus-one-query" => "If the matched loop performs one uncached remote query per item, database work grows with result size; measurement distinguishes that runtime behavior from a bounded loop or request-scoped batch/cache.",
        "no-pagination" => "If no wrapper, database view, or domain invariant supplies a bound, a collection read makes query work, memory, serialization, and response size grow with the dataset.",
        "supabase-rls-missing" => "If the deployed client-accessible table truly lacks Row Level Security, database grants rather than per-row policies govern anon and authenticated access; local artifact absence alone does not establish deployed state.",
        "supabase-policy-set-empty" => "RLS with no applicable policies is default-deny for ordinary client roles, so the detected browser operation may be unavailable if the applied policy state matches the scanned local artifacts.",
        "supabase-policy-operation-missing" => "With RLS enabled, a missing policy for the operation denies that client action; the finding points to functionality or project/deployed policy drift, not an access bypass.",
        "supabase-service-role-client" => "A live Supabase service-role credential in a browser bundle would bypass RLS and transfer privileged access to visitors, but a source reference alone does not prove the configured value reached that bundle.",

        // Reliability and operational visibility.
        "external-call-timeout" => "A remote call without an application deadline can retain request, connection, and worker capacity until a lower-layer timeout intervenes when the dependency stalls.",
        "external-call-retry" => "Zero retries is a valid policy. When retries are enabled, the effective policy must be bounded, deadline-aware, and idempotency-aware so recovery does not duplicate side effects or amplify an outage.",
        "healthcheck-missing" => "When a deployment relies on an application health endpoint, distinct liveness and readiness signals let it restart an unresponsive process without restarting healthy instances merely because a shared dependency is unavailable.",
        "structured-logging-missing" => "For server work without equivalent runtime or platform instrumentation, consistent structured fields let operators correlate requests, filter failures, and alert across instances without parsing ad hoc text.",
        "job-visibility-missing" => "Operationally important background work needs enough provider, log, metric, or durable-state visibility to detect stuck, retried, or terminally failed jobs; the necessary mechanism depends on the job system and delivery guarantees.",
        "error-boundary-missing" => "A component boundary can contain descendant rendering failures and offer recovery, but it is only one part of error handling and does not catch event-handler, asynchronous, server-rendering, or boundary-internal failures.",
        "console-log-error-handling" => "Logging an exception without returning a controlled error, retrying, or restoring state can leave the caller believing an operation completed when it did not.",
        "critical-path-no-test" => "A regression in authentication, payment, or another critical path has disproportionate impact, and an automated behavior test is the repeatable evidence that the boundary still holds.",
        "nextconfig-errors-ignored" => "Disabling build-time type or lint failures allows known diagnostics to ship and also hides new regressions behind the same global exception.",

        // Maintainability and project structure.
        "jsx-inline-style-density" => "Repeated static style objects make shared visual changes and state variants harder to review; genuinely dynamic values can remain inline.",
        "typescript-any-abuse" => "Heavy use of `any` removes compiler checks at the exact boundaries where value shape mistakes otherwise surface before runtime.",
        "empty-catch-blocks" => "Silently swallowing an error erases both the failure signal and the caller's chance to recover, while execution continues with potentially incomplete state.",
        "placeholder-density" => "A concentration of placeholder-style tokens merits triage because some may represent unresolved high-risk work, while the count alone does not establish meaning, authorship, reachability, or launch impact.",
        "oversized-module" | "god-module" => "A module with many unrelated responsibilities increases change coupling and makes security, error, and data boundaries harder to review in isolation.",
        "god-route" => "A route that combines parsing, policy, persistence, external calls, and response shaping makes failure and authorization paths difficult to test independently.",
        "db-scattered-across-routes" => "Repeated direct database access across routes can make tenant, transaction, retry, and query policy drift more likely, but import placement alone does not prove those policies are inconsistent or that a repository layer is required.",

        // Environment and repository readiness.
        "hardcoded-localhost-url" => "A fixed loopback destination can become an environment-specific failure when the service moves across hosts or containers, but it may be intentional for a co-located sidecar or local-only path and needs topology review.",
        "env-example-missing" => "A secret-free inventory of developer-supplied environment names makes clean setup reproducible while distinguishing required, optional, defaulted, platform-injected, and externally managed configuration.",

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::LazyLock;

    static BUILD_ISSUE_SLUG: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"(?s)build_issue\(\s*"([^"]+)""#)
            .expect("static build_issue slug regex") // allow-expect: compile-time literal regex
    });

    const BUILD_ISSUE_SOURCES: &[&str] = &[
        include_str!("file_analysis/ai_checks.rs"),
        include_str!("file_analysis/architecture_checks.rs"),
        include_str!("file_analysis/js_sinks.rs"),
        include_str!("file_analysis/php_sinks.rs"),
        include_str!("file_analysis/python_sinks.rs"),
        include_str!("file_analysis/route_security.rs"),
        include_str!("file_analysis/service_security.rs"),
        include_str!("operations/app_readiness.rs"),
        include_str!("operations/env_checks.rs"),
        include_str!("operations/framework_debug.rs"),
        include_str!("operations/project_hygiene.rs"),
        include_str!("operations/supabase_policies.rs"),
    ];

    #[test]
    fn every_literal_build_issue_slug_has_specific_rationale() {
        let mut slugs = BUILD_ISSUE_SOURCES
            .iter()
            .flat_map(|source| {
                BUILD_ISSUE_SLUG
                    .captures_iter(source)
                    .map(|capture| capture[1].to_string())
            })
            .collect::<BTreeSet<_>>();
        // route_security chooses one of these two slugs in the first argument.
        slugs.insert("ai-request-validation".into());
        slugs.insert("request-validation".into());

        let missing = slugs
            .iter()
            .filter(|slug| code_issue_rationale(slug).is_none())
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "build_issue slugs missing rationale: {missing:?}"
        );

        for slug in slugs {
            let rationale = code_issue_rationale(&slug).expect("mapped rationale");
            assert!(
                rationale.len() >= 60,
                "{slug} rationale is too thin: {rationale}"
            );
            assert!(!rationale.contains("main flow is usually in place"));
        }
    }

    #[test]
    fn unrelated_rules_do_not_receive_the_same_generic_rationale() {
        assert_ne!(
            code_issue_rationale("raw-sql-unsafe"),
            code_issue_rationale("jsx-inline-style-density")
        );
        assert!(code_issue_rationale("raw-sql-unsafe")
            .expect("SQL rationale")
            .contains("SQL"));
    }
}
