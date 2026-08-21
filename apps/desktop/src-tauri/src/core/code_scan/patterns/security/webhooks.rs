use std::sync::LazyLock;

// Require structural signature validation; generic webhook or HMAC references are insufficient.
pub(in crate::core::code_scan) static WEBHOOK_VERIFY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            // Match the construct*Event* verification family used across
            // Stripe, Svix, Clerk, and compatible providers.
            regex::Regex::new(r"\bconstruct\w*Event\w*\s*\(").unwrap(),
            regex::Regex::new(r"\bwebhook\.verify\s*\(").unwrap(),
            regex::Regex::new(r"\bwebhooks\.unwrap\s*\(").unwrap(),
            regex::Regex::new(r"\bsvix\.").unwrap(),
            regex::Regex::new(r"new\s+Webhook\s*\(").expect("static Webhook ctor regex"), // allow-expect: compile-time literal regex
            // Signature header read AND compared
            regex::Regex::new(r#"(?i)Stripe-Signature"#).expect("static stripe signature regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?i)x-(?:hub|line|github|gitlab|slack)-signature"#)
                .expect("static x-*-signature regex"), // allow-expect: compile-time literal regex
            // HMAC compute over request body specifically
            regex::Regex::new(r"createHmac\s*\(").expect("static createHmac regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bhmac\b\s*\(").expect("static hmac call regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"timingSafeEqual\s*\(").expect("static timingSafeEqual regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bverify(?:Signature|Webhook)\s*\(").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static IDEMPOTENCY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bidempot").unwrap(),
            regex::Regex::new(r"\balreadyProcessed\b").unwrap(),
            regex::Regex::new(r"\bprocessedEvent\b").unwrap(),
            regex::Regex::new(r"\beventId\b").unwrap(),
            regex::Regex::new(r"ON CONFLICT").unwrap(),
            regex::Regex::new(r"upsert\s*\(").unwrap(),
            regex::Regex::new(r"insertOrIgnore").unwrap(),
            regex::Regex::new(r"firstOrCreate").unwrap(),
            regex::Regex::new(r"findUnique\s*\(").unwrap(),
        ]
    });
