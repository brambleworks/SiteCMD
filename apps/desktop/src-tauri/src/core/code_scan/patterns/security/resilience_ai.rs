use std::sync::LazyLock;

// Require structural outbound timeout patterns; generic timeout text is insufficient.
pub(in crate::core::code_scan) static TIMEOUT_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bnew\s+AbortController\b").unwrap(),
            regex::Regex::new(r"\babortSignal\b").unwrap(),
            // Explicit timeout option with a numeric value
            regex::Regex::new(r"(?i)\btimeout\s*:\s*\d+").unwrap(),
            regex::Regex::new(r"(?i)\btimeoutMs\s*:\s*\d+").unwrap(),
            regex::Regex::new(r"(?i)\brequestTimeout\s*:\s*\d+").expect("static timeout regex"), // allow-expect: compile-time literal regex
            // signal: prop on fetch/axios
            regex::Regex::new(r"\bsignal\s*:\s*\w").unwrap(),
            // Promise.race with a timeout pattern
            regex::Regex::new(r"Promise\.race\s*\(").unwrap(),
            // AbortSignal.timeout helper (modern API)
            regex::Regex::new(r"AbortSignal\.timeout\s*\(").expect("static abort-signal regex"), // allow-expect: compile-time literal regex
        ]
    });

/// Runtime-specific patterns that explicitly disable outbound TLS verification.
pub(in crate::core::code_scan) static TLS_VERIFICATION_DISABLED_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"rejectUnauthorized\s*:\s*false")
            .expect("static rejectUnauthorized regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r#"NODE_TLS_REJECT_UNAUTHORIZED["']?\s*\]?\s*[=:]\s*["']?0\b"#)
            .expect("static NODE_TLS regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(
            r"(?s)\b(?:requests|httpx)\.(?:get|post|put|patch|delete|head|options|request|stream|Client|AsyncClient)\s*\([^)]{0,300}verify\s*=\s*False",
        )
        .expect("static requests verify regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r"\.verify\s*=\s*False\b")
            .expect("static session verify regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r"CURLOPT_SSL_VERIFY(?:PEER|HOST)\s*(?:,|=>)\s*(?:false|0)\b")
            .expect("static curlopt regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r"InsecureSkipVerify\s*:\s*true")
            .expect("static Go tls regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r"OpenSSL::SSL::VERIFY_NONE")
            .expect("static Ruby verify-none regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r#"\bcurl\b[^\n]{0,120}(?:--insecure\b|\s-k\s+["']?https?://)"#)
            .expect("static curl insecure regex"), // allow-expect: compile-time literal regex
    ]
});

pub(in crate::core::code_scan) static RETRY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bretry\b").unwrap(),
            regex::Regex::new(r"\bbackoff\b").unwrap(),
            regex::Regex::new(r"\bmaxRetries\b").unwrap(),
            regex::Regex::new(r"\battempts?\b").unwrap(),
            regex::Regex::new(r"exponential").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static CONCURRENCY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"p-limit").unwrap(),
            regex::Regex::new(r"bottleneck").unwrap(),
            regex::Regex::new(r"\bsemaphore\b").unwrap(),
            regex::Regex::new(r"\bmaxConcurrency\b").unwrap(),
            regex::Regex::new(r"\bconcurrency\b").unwrap(),
            regex::Regex::new(r"queue\.add").unwrap(),
            regex::Regex::new(r"limiter\.schedule").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LLM_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"from\s+["']ai["']"#).unwrap(),
            regex::Regex::new(r#"require\(\s*["']ai["']\s*\)"#).unwrap(),
            regex::Regex::new(r#"from\s+["']openai["']"#).unwrap(),
            regex::Regex::new(r#"require\(\s*["']openai["']\s*\)"#).unwrap(),
            regex::Regex::new(r"\bnew\s+OpenAI\s*\(").unwrap(),
            regex::Regex::new(r#"from\s+["']@ai-sdk/openai["']"#).unwrap(),
            regex::Regex::new(r#"from\s+["']@anthropic-ai/sdk["']"#).unwrap(),
            regex::Regex::new(r#"require\(\s*["']@anthropic-ai/sdk["']\s*\)"#).unwrap(),
            regex::Regex::new(r"\bnew\s+Anthropic\s*\(").unwrap(),
            regex::Regex::new(r#"from\s+["']@ai-sdk/anthropic["']"#).unwrap(),
            regex::Regex::new(r"api\.openai\.com").unwrap(),
            regex::Regex::new(r"api\.anthropic\.com").unwrap(),
            regex::Regex::new(r"@google/generative-ai").unwrap(),
            regex::Regex::new(r#"from\s+["']@ai-sdk/google["']"#).unwrap(),
            regex::Regex::new(r"google\.generativeai").unwrap(),
            regex::Regex::new(r"generateContent").unwrap(),
            regex::Regex::new(r"chat\.completions\.create").unwrap(),
            regex::Regex::new(r"responses\.create").unwrap(),
            regex::Regex::new(r"messages\.create").unwrap(),
            regex::Regex::new(r"embeddings\.create").unwrap(),
            regex::Regex::new(r"\bstreamText\s*\(").unwrap(),
            regex::Regex::new(r"\bgenerateText\s*\(").unwrap(),
            regex::Regex::new(r"\bgenerateObject\s*\(").unwrap(),
            regex::Regex::new(r"\bstreamObject\s*\(").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LLM_SPEND_GUARD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bbudget\b").unwrap(),
            regex::Regex::new(r"\bquota\b").unwrap(),
            // `credits?` (not `credit(s)?`): same regex semantics, and the
            // grouped form false-trips the lazy-plural copy guardrail.
            regex::Regex::new(r"\bcredits?\b").unwrap(),
            regex::Regex::new(r"\busageLimit\b").unwrap(),
            regex::Regex::new(r"\bdailyLimit\b").unwrap(),
            regex::Regex::new(r"\bmonthlyLimit\b").unwrap(),
            regex::Regex::new(r"\bmaxCost\b").unwrap(),
            regex::Regex::new(r"\btokenBudget\b").unwrap(),
            regex::Regex::new(r"\bspend\b").unwrap(),
            regex::Regex::new(r"\bremainingTokens\b").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LLM_USAGE_LOGGING_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"total_tokens").unwrap(),
            regex::Regex::new(r"prompt_tokens").unwrap(),
            regex::Regex::new(r"completion_tokens").unwrap(),
            regex::Regex::new(r"output_tokens").unwrap(),
            regex::Regex::new(r"totalTokens").unwrap(),
            regex::Regex::new(r"inputTokens").unwrap(),
            regex::Regex::new(r"outputTokens").unwrap(),
            regex::Regex::new(r"reasoningTokens").unwrap(),
            regex::Regex::new(r"estimatedCost").unwrap(),
            regex::Regex::new(r"usage\??\.").unwrap(),
            regex::Regex::new(r"result\.usage").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LLM_OBSERVABILITY_CONTEXT_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"request_id").unwrap(),
        regex::Regex::new(r"\brequestId\b").unwrap(),
        regex::Regex::new(r"x-request-id").unwrap(),
        regex::Regex::new(r"_request_id").unwrap(),
        regex::Regex::new(r"providerMetadata").unwrap(),
        regex::Regex::new(r"experimental_telemetry").unwrap(),
        regex::Regex::new(r"\bonFinish\b").unwrap(),
        regex::Regex::new(r"\bonStepFinish\b").unwrap(),
    ]
});

pub(in crate::core::code_scan) static AI_OBSERVABILITY_INTEGRATION_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"\blangfuse\b").unwrap(),
        regex::Regex::new(r"\blangsmith\b").unwrap(),
        regex::Regex::new(r"\bhelicone\b").unwrap(),
        regex::Regex::new(r"\bbraintrust\b").unwrap(),
        regex::Regex::new(r"\bhumanloop\b").unwrap(),
        regex::Regex::new(r"\bportkey\b").unwrap(),
        regex::Regex::new(r"\btraceloop\b").unwrap(),
        regex::Regex::new(r"\bopentelemetry\b").unwrap(),
        regex::Regex::new(r"\bx-helicone-").unwrap(),
    ]
});

pub(in crate::core::code_scan) static LLM_TOKEN_CAP_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"max_output_tokens").unwrap(),
            regex::Regex::new(r"max_completion_tokens").unwrap(),
            regex::Regex::new(r"max_tokens").unwrap(),
            regex::Regex::new(r"maxTokens").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LLM_CACHE_DEDUPE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bunstable_cache\b").unwrap(),
            regex::Regex::new(r"\bpromptCache\b").unwrap(),
            regex::Regex::new(r"\bcacheKey\b").unwrap(),
            regex::Regex::new(r"\bidempotencyKey\b").unwrap(),
            regex::Regex::new(r"\bdedupe\b").unwrap(),
            regex::Regex::new(r"\bmemoize\b").unwrap(),
            regex::Regex::new(r"\blru\b").unwrap(),
            regex::Regex::new(r"\bcreateHash\b").unwrap(),
            regex::Regex::new(r"\bsha256\b").unwrap(),
            regex::Regex::new(r"\bredis\b").unwrap(),
            regex::Regex::new(r"\bupstash\b").unwrap(),
            regex::Regex::new(r"\bkv\.(get|set)\b").unwrap(),
            regex::Regex::new(r"\bgetOrSet\b").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static USER_CONTROLLED_LLM_MODEL_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(
            r#"(?is)model\s*:\s*(?:body|input|payload|params|query|data)\.[A-Za-z0-9_]*model[A-Za-z0-9_]*"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)(?:openai|anthropic|google|createOpenAI|createAnthropic|createGoogleGenerativeAI)\s*\(\s*(?:body|input|payload|params|query|data)\.[A-Za-z0-9_]*model[A-Za-z0-9_]*\s*\)"#,
        )
        .unwrap(),
        regex::Regex::new(r#"(?is)searchParams\.get\(\s*["'`]model["'`]\s*\)"#).unwrap(),
        regex::Regex::new(
            r#"(?is)(?:formData|data|input|values)\.get\s*\(\s*["'`](?:model|provider)["'`]\s*\)"#,
        )
        .unwrap(),
        regex::Regex::new(r#"(?is)\breq\.query\.(?:model|provider)\b"#).unwrap(),
    ]
});

pub(in crate::core::code_scan) static USER_CONTROLLED_LLM_SETTING_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(
            r#"(?is)(?:maxTokens|max_tokens|max_output_tokens|max_completion_tokens|temperature|topP|top_p|presencePenalty|frequencyPenalty)\s*:\s*(?:body|input|payload|params|query|data)\.[A-Za-z0-9_]+"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)(?:maxTokens|max_tokens|max_output_tokens|max_completion_tokens|temperature|topP|top_p|presencePenalty|frequencyPenalty)\s*:\s*.*searchParams\.get\("#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)(?:formData|data|input|values)\.get\s*\(\s*["'`](?:maxTokens|max_tokens|max_output_tokens|max_completion_tokens|temperature|topP|top_p|presencePenalty|frequencyPenalty)["'`]\s*\)"#,
        )
        .unwrap(),
    ]
});

pub(in crate::core::code_scan) static MODEL_ALLOWLIST_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bz\.enum\s*\(").unwrap(),
            regex::Regex::new(r"\ballowedModels\b").unwrap(),
            regex::Regex::new(r"\bsupportedModels\b").unwrap(),
            regex::Regex::new(r"\bMODEL_MAP\b").unwrap(),
            regex::Regex::new(r"\bMODEL_ALLOWLIST\b").unwrap(),
            regex::Regex::new(
                r"\bincludes\s*\(\s*(?:body|input|payload|params|query|data)\.[A-Za-z0-9_]*model",
            )
            .unwrap(),
            regex::Regex::new(
                r"\bswitch\s*\(\s*(?:body|input|payload|params|query|data)\.[A-Za-z0-9_]*model",
            )
            .unwrap(),
            regex::Regex::new(r"\bassertValidModel\b").unwrap(),
            regex::Regex::new(r"\bparseModel\b").unwrap(),
        ]
    });

/// Request-derived output-length fields. These can expand billable output and
/// latency up to the provider limit, so they grade above sampling-only knobs
/// such as temperature or top-p when no effective product bound is visible.
pub(in crate::core::code_scan) static USER_CONTROLLED_LLM_OUTPUT_LIMIT_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(
            r#"(?is)(?:maxTokens|max_tokens|max_output_tokens|max_completion_tokens)\s*:\s*(?:body|input|payload|params|query|data)\.[A-Za-z0-9_]+"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)(?:maxTokens|max_tokens|max_output_tokens|max_completion_tokens)\s*:\s*.*searchParams\.get\("#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)(?:formData|data|input|values)\.get\s*\(\s*["'`](?:maxTokens|max_tokens|max_output_tokens|max_completion_tokens)["'`]\s*\)"#,
        )
        .unwrap(),
    ]
});

pub(in crate::core::code_scan) static NUMERIC_BOUND_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bMath\.min\s*\(").unwrap(),
            regex::Regex::new(r"\bMath\.max\s*\(").unwrap(),
            regex::Regex::new(r"\bclamp\s*\(").unwrap(),
            regex::Regex::new(r"\.min\s*\(").unwrap(),
            regex::Regex::new(r"\.max\s*\(").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LOOP_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"while\s*\(\s*true\s*\)").unwrap(),
            regex::Regex::new(r"for\s*\(\s*;\s*;\s*\)").unwrap(),
            regex::Regex::new(r"setInterval\s*\(").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LOOP_GUARD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bmaxAttempts\b").unwrap(),
            regex::Regex::new(r"\bmaxIterations\b").unwrap(),
            regex::Regex::new(r"\bclearInterval\b").unwrap(),
            regex::Regex::new(r"\bcancel(l?ed)?\b").unwrap(),
            regex::Regex::new(r"\bshouldContinue\b").unwrap(),
            regex::Regex::new(r"\bstop\b").unwrap(),
        ]
    });
