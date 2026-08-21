use std::sync::LazyLock;

pub(in crate::core::code_scan) static DANGEROUS_HTML_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"dangerouslySetInnerHTML").unwrap(),
            regex::Regex::new(r"v-html").unwrap(),
            // Flag only dynamic innerHTML assignments; static and empty values
            // are not XSS evidence.
            regex::Regex::new(r"innerHTML\s*=[^;]*\$\{").unwrap(),
            regex::Regex::new(r"innerHTML\s*=[^;]*\+").unwrap(),
            // `Markup(` with a word boundary so Drupal's safe wrappers
            // (TranslatableMarkup, FormattableMarkup, PlaceholderMarkup, …)
            // don't trip the check on every plugin metadata line.
            regex::Regex::new(r"\bMarkup\(").unwrap(),
            // PHP: a request superglobal reaching output directly - echo,
            // print, the short echo tag, or Blade's raw-output braces.
            // Escaped output also matches, but the sanitization patterns
            // suppress the check for those files.
            regex::Regex::new(
                r"(?:\becho\b|\bprint\b|<\?=|\{!!)[^;]{0,160}\$_(?:GET|POST|REQUEST|COOKIE|SERVER)",
            )
            .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static JSX_INLINE_STYLE_PROP_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"style\s*=\s*\{\s*\{").unwrap());

pub(in crate::core::code_scan) static SANITIZATION_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"DOMPurify").unwrap(),
            regex::Regex::new(r"sanitizeHtml").unwrap(),
            regex::Regex::new(r"\bsanitize\b").unwrap(),
            regex::Regex::new(r"\bbleach\b").unwrap(),
            regex::Regex::new(r"esc_html").unwrap(),
            regex::Regex::new(r"\bescapeHtml\b").unwrap(),
            regex::Regex::new(r"\bescape_html\b").unwrap(),
            // Drupal-specific sanitization helpers
            regex::Regex::new(r"Html::escape\b").unwrap(),
            regex::Regex::new(r"Html::format\b").unwrap(),
            regex::Regex::new(r"Xss::filter\b").unwrap(),
            regex::Regex::new(r"Xss::filterAdmin\b").unwrap(),
            // PHP native escaping and the WordPress esc_*/wp_kses/sanitize_*
            // families (esc_html is already covered above).
            regex::Regex::new(r"\bhtmlspecialchars\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bhtmlentities\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\besc_attr").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\besc_url").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bwp_kses").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bsanitize_[a-z_]+\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static SSRF_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(
    || {
        vec![
            regex::Regex::new(
                r"fetch\s*\(\s*(?:url|body\.url|req\.body\.url|requestUrl|targetUrl)",
            )
            .unwrap(),
            regex::Regex::new(
                r"axios\.(get|post)\s*\(\s*(?:url|body\.url|req\.body\.url|requestUrl|targetUrl)",
            )
            .unwrap(),
            regex::Regex::new(
                r#"requests\.(get|post)\s*\(\s*(?:url|data\[['"]url['"]\]|target_url)"#,
            )
            .unwrap(),
            // Direct request-accessor arguments - fetch(req.query.url) and
            // requests.get(request.args["url"]) produced no finding when
            // only fixed variable names were matched.
            regex::Regex::new(r"fetch\s*\(\s*req(?:uest)?\.(?:query|body|params)\.[A-Za-z0-9_]+")
                .expect("static SSRF pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"axios\.(?:get|post)\s*\(\s*req(?:uest)?\.(?:query|body|params)\.[A-Za-z0-9_]+",
            )
            .expect("static SSRF pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"fetch\s*\(\s*searchParams\.get\s*\(")
                .expect("static SSRF pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"requests\.(?:get|post)\s*\(\s*request\.(?:args|form|values|GET|POST)(?:\.get\s*\(|\s*\[)",
            )
            .expect("static SSRF pattern regex"), // allow-expect: compile-time literal regex
        ]
    },
);

pub(in crate::core::code_scan) static SSRF_GUARD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"allowlist").unwrap(),
            regex::Regex::new(r"hostname").unwrap(),
            regex::Regex::new(r"host_whitelist").unwrap(),
            regex::Regex::new(r"new URL").unwrap(),
            regex::Regex::new(r"urlparse").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static HARDCODED_SECRET_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)("|')sk[-_]live[-_][a-zA-Z0-9]{20,}("|')"#).unwrap(),
            regex::Regex::new(r#"(?i)("|')sk[-_]test[-_][a-zA-Z0-9]{20,}("|')"#).unwrap(),
            regex::Regex::new(r#"("|')AKIA[0-9A-Z]{16}("|')"#).unwrap(),
            regex::Regex::new(r#"(?i)("|')sk-[a-zA-Z0-9]{20,}("|')"#).unwrap(),
            regex::Regex::new(r#"(?i)("|')ghp_[a-zA-Z0-9]{36,}("|')"#).unwrap(),
            regex::Regex::new(r#"(?i)("|')gho_[a-zA-Z0-9]{36,}("|')"#).unwrap(),
            regex::Regex::new(r#"(?i)("|')glpat-[a-zA-Z0-9\-_]{20,}("|')"#).unwrap(),
            regex::Regex::new(r#"(?i)("|')xox[bprs]-[a-zA-Z0-9\-]{10,}("|')"#).unwrap(),
            regex::Regex::new(r#"(?i)("|')SG\.[a-zA-Z0-9\-_]{22,}("|')"#).unwrap(),
            // Match unambiguous PEM armor across quoted, heredoc, template, and
            // escaped-newline representations.
            regex::Regex::new(r"-----BEGIN (?:[A-Z]+ )*PRIVATE KEY(?: BLOCK)?-----")
                .expect("static PEM armor regex"), // allow-expect: compile-time literal regex
            // Google API key (also matched by the web-side exposed_keys check;
            // the source path was blind to it).
            regex::Regex::new(r#"("|')AIza[0-9A-Za-z_\-]{35}("|')"#)
                .expect("static AIza key regex"), // allow-expect: compile-time literal regex
            // npm granular access token.
            regex::Regex::new(r#"("|')npm_[A-Za-z0-9]{36}("|')"#).expect("static npm token regex"), // allow-expect: compile-time literal regex
            // Stripe restricted key (sk_live/sk_test are covered above).
            regex::Regex::new(r#"(?i)("|')rk[-_]live[-_][a-zA-Z0-9]{20,}("|')"#)
                .expect("static Stripe restricted key regex"), // allow-expect: compile-time literal regex
            // OpenAI project-scoped key: the hyphen after "proj" breaks the
            // generic sk- pattern's contiguous [a-zA-Z0-9]{20,} run.
            regex::Regex::new(r#"("|')sk-proj-[a-zA-Z0-9_\-]{20,}("|')"#)
                .expect("static OpenAI project key regex"), // allow-expect: compile-time literal regex
            // GitHub fine-grained personal access token.
            regex::Regex::new(r#"("|')github_pat_[A-Za-z0-9_]{22,}("|')"#)
                .expect("static GitHub fine-grained PAT regex"), // allow-expect: compile-time literal regex
        ]
    });

/// Weak placeholder credentials reported separately from exposed secrets.
pub(in crate::core::code_scan) static WEAK_DEFAULT_CREDENTIAL_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?i)("|')supersecretkey("|')"#).unwrap(),
        regex::Regex::new(r#"(?i)("|')changeme("|')"#).unwrap(),
        regex::Regex::new(r#"(?i)("|')password123("|')"#).unwrap(),
    ]
});

pub(in crate::core::code_scan) static EMPTY_CATCH_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"catch\s*\([^)]*\)\s*\{\s*\}").unwrap());

pub(in crate::core::code_scan) static CONSOLE_LOG_CATCH_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(
            r"catch\s*\([^)]*\)\s*\{\s*console\.(log|warn|error)\s*\([^)]*\)\s*;?\s*\}",
        )
        .unwrap()
    });

pub(in crate::core::code_scan) static AI_COMMENT_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(
                r"(?i)//\s*(as an ai|here'?s how|i'?ll help|let me know if|sure,? here)",
            )
            .unwrap(),
            regex::Regex::new(
                r"(?i)//\s*(i'?ve (created|generated|written|added|implemented|set up))",
            )
            .unwrap(),
            regex::Regex::new(
                r"(?i)//\s*this (code|function|component|module) (was generated|is generated) by",
            )
            .unwrap(),
            regex::Regex::new(r"(?i)/\*\s*(as an ai|here'?s how|i'?ll help|sure,? here)").unwrap(),
            regex::Regex::new(r"(?i)#\s*(as an ai|here'?s how|i'?ll help|sure,? here)").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LOCALHOST_URL_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)("|')(https?://localhost[:/][^"']*)("|')"#).unwrap(),
            regex::Regex::new(r#"(?i)("|')(https?://127\.0\.0\.1[:/][^"']*)("|')"#).unwrap(),
            regex::Regex::new(r"`https?://localhost[:/]").unwrap(),
            regex::Regex::new(r"`https?://127\.0\.0\.1[:/]").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static PLACEHOLDER_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        // Word boundaries keep placeholder-density evidence from matching
        // ordinary identifiers that merely contain a marker substring.
        vec![regex::Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX|CHANGEME|PLACEHOLDER)\b").unwrap()]
    });

pub(in crate::core::code_scan) static CLIENT_ENV_SECRET_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"(?i)NEXT_PUBLIC_[A-Z0-9_]*(?:SECRET|KEY|TOKEN|PASSWORD|PRIVATE)")
                .unwrap(),
            regex::Regex::new(r"(?i)VITE_[A-Z0-9_]*(?:SECRET|KEY|TOKEN|PASSWORD|PRIVATE)").unwrap(),
            regex::Regex::new(r"(?i)REACT_APP_[A-Z0-9_]*(?:SECRET|KEY|TOKEN|PASSWORD|PRIVATE)")
                .unwrap(),
            regex::Regex::new(r"(?i)EXPO_PUBLIC_[A-Z0-9_]*(?:SECRET|KEY|TOKEN|PASSWORD|PRIVATE)")
                .unwrap(),
        ]
    });

pub(in crate::core::code_scan) static CLIENT_ENV_SECRET_REFERENCE_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(
            r#"(?i)\b(?:process\.env|import\.meta\.env)\.((?:NEXT_PUBLIC|VITE|REACT_APP|EXPO_PUBLIC)_[A-Z0-9_]*(?:SECRET|KEY|TOKEN|PASSWORD|PRIVATE))\b"#,
        )
        .expect("static client env secret dot-access regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(
            r#"(?i)\b(?:process\.env|import\.meta\.env)\[\s*["']((?:NEXT_PUBLIC|VITE|REACT_APP|EXPO_PUBLIC)_[A-Z0-9_]*(?:SECRET|KEY|TOKEN|PASSWORD|PRIVATE))["']\s*\]"#,
        )
        .expect("static client env secret bracket-access regex"), // allow-expect: compile-time literal regex
    ]
});

/// Public-by-design client environment names that require backend controls but
/// are not secrets by themselves.
pub(in crate::core::code_scan) static CLIENT_ENV_PUBLIC_ALLOWLIST_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        // Stripe publishable keys: NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY etc.
        regex::Regex::new(r"(?i)\b(?:NEXT_PUBLIC|VITE|REACT_APP|EXPO_PUBLIC)_[A-Z0-9_]*PUBLISHABLE[A-Z0-9_]*\b")
            .expect("static publishable allowlist regex"), // allow-expect: compile-time literal regex
        // Supabase anon JWT keys: NEXT_PUBLIC_SUPABASE_ANON_KEY etc.
        regex::Regex::new(r"(?i)\b(?:NEXT_PUBLIC|VITE|REACT_APP|EXPO_PUBLIC)_[A-Z0-9_]*ANON[A-Z0-9_]*\b")
            .expect("static anon allowlist regex"), // allow-expect: compile-time literal regex
        // Generic "PUBLIC_KEY" naming (e.g. NEXT_PUBLIC_POSTHOG_PUBLIC_KEY).
        regex::Regex::new(r"(?i)\b(?:NEXT_PUBLIC|VITE|REACT_APP|EXPO_PUBLIC)_[A-Z0-9_]*PUBLIC_KEY\b")
            .expect("static public_key allowlist regex"), // allow-expect: compile-time literal regex
        // OAuth client IDs are designed to be public (NEXT_PUBLIC_GOOGLE_CLIENT_ID etc.).
        regex::Regex::new(r"(?i)\b(?:NEXT_PUBLIC|VITE|REACT_APP|EXPO_PUBLIC)_[A-Z0-9_]*CLIENT_ID\b")
            .expect("static client_id allowlist regex"), // allow-expect: compile-time literal regex
        // PostHog / Plausible / similar product-analytics public-token convention.
        regex::Regex::new(r"(?i)\b(?:NEXT_PUBLIC|VITE|REACT_APP|EXPO_PUBLIC)_(?:POSTHOG|PLAUSIBLE|UMAMI|FATHOM|SIMPLEANALYTICS|MIXPANEL)_[A-Z0-9_]*(?:KEY|TOKEN|ID)\b")
            .expect("static product-analytics allowlist regex"), // allow-expect: compile-time literal regex
        // Captcha SITE keys (hCaptcha / reCAPTCHA / Cloudflare Turnstile) are the
        // public half of the pair, embedded in the page by design; and web-map
        // keys (Google Maps, Mapbox) are public, referrer-restricted client keys:
        // NEXT_PUBLIC_HCAPTCHA_SITE_KEY, NEXT_PUBLIC_GOOGLE_MAPS_KEY, etc.
        regex::Regex::new(r"(?i)\b(?:NEXT_PUBLIC|VITE|REACT_APP|EXPO_PUBLIC)_[A-Z0-9_]*(?:SITE_KEY|MAPS[A-Z0-9_]*KEY|MAPBOX[A-Z0-9_]*)\b")
            .expect("static site/map key allowlist regex"), // allow-expect: compile-time literal regex
    ]
});

pub(in crate::core::code_scan) static TYPESCRIPT_ANY_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r":\s*any\b").unwrap());

pub(in crate::core::code_scan) static EVAL_EXEC_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\beval\s*\(").unwrap(),
            regex::Regex::new(r"\bnew\s+Function\s*\(").unwrap(),
        ]
    });

// These are call patterns only; shell-injection also requires a child_process
// import. Python sinks are graded by their dedicated check.
pub(in crate::core::code_scan) static EXEC_SPAWN_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"(?i)\bexec\s*\(").unwrap(),
            regex::Regex::new(r"(?i)\bexecSync\s*\(").unwrap(),
            regex::Regex::new(r"(?i)\bspawn\s*\(").unwrap(),
            regex::Regex::new(r"(?i)\bspawnSync\s*\(").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static EXEC_SAFE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"(?i)shellEscape").unwrap(),
            regex::Regex::new(r"(?i)shell_escape").unwrap(),
            regex::Regex::new(r"(?i)escapeshell").unwrap(),
            regex::Regex::new(r"(?i)shlex\.quote").unwrap(),
            regex::Regex::new(r"(?i)shlex\.split").unwrap(),
            regex::Regex::new(r"(?i)shellescape").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LOCALSTORAGE_AUTH_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(r#"(?i)localStorage\.(setItem|getItem)\s*\(\s*("|')(token|auth|jwt|access_token|refresh_token|session|id_token|api_key|apiKey|bearer)("|')"#).unwrap(),
        regex::Regex::new(r#"(?i)sessionStorage\.(setItem|getItem)\s*\(\s*("|')(token|auth|jwt|access_token|refresh_token|session|id_token|api_key|apiKey|bearer)("|')"#).unwrap(),
    ]
    });

// Require structural pagination syntax; bare words appear in unrelated code.
pub(in crate::core::code_scan) static PAGINATION_GUARD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            // Object-literal keys with a value: `take: 50`, `limit: 100`
            regex::Regex::new(r"(?i)\btake\s*:\s*\d+").unwrap(),
            regex::Regex::new(r"(?i)\blimit\s*:\s*\d+").unwrap(),
            regex::Regex::new(r"(?i)\boffset\s*:\s*\d+").unwrap(),
            regex::Regex::new(r"(?i)\bpageSize\s*:\s*\d+").unwrap(),
            regex::Regex::new(r"(?i)\bpage_size\s*:\s*\d+").unwrap(),
            regex::Regex::new(r"(?i)\bperPage\s*:\s*\d+").unwrap(),
            regex::Regex::new(r"(?i)\bper_page\s*:\s*\d+").unwrap(),
            regex::Regex::new(
                r"(?i)\b(?:take|limit|offset|pageSize|page_size|perPage|per_page)\s*:\s*[A-Za-z_$][A-Za-z0-9_$]*",
            )
            .expect("static variable pagination regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"(?i)[{,]\s*(?:take|limit|offset|pageSize|page_size|perPage|per_page|cursor)\s*[,}]",
            )
            .expect("static shorthand pagination regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\bLIMIT\s+\d").unwrap(),
            // Explicit pagination method calls
            regex::Regex::new(r"(?i)\bpaginate\s*\(").unwrap(),
            regex::Regex::new(r"(?i)\.cursor\s*\(").unwrap(),
            // Cursor as an object-literal key (Prisma-style)
            regex::Regex::new(r"(?i)\bcursor\s*:\s*\{").expect("static cursor-key regex"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static LIST_QUERY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\.findMany\s*\(").unwrap(),
            regex::Regex::new(r"\.find\s*\(\s*\{").unwrap(),
            regex::Regex::new(r"\.select\s*\(\s*\)").unwrap(),
            regex::Regex::new(r"(?i)SELECT\s+.+\s+FROM\s+").unwrap(),
            regex::Regex::new(r"\.query\.").unwrap(),
            regex::Regex::new(r"\.getAll\s*\(").unwrap(),
            regex::Regex::new(r"\.list\s*\(").unwrap(),
            regex::Regex::new(r"\.scan\s*\(").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static PASSWORD_STORE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        // Limit plaintext-password detection to ORM or SQL writes; ordinary form
        // value objects are not evidence of storage.
        vec![
            regex::Regex::new(r"(?i)\.create\s*\([^)]*password\s*:").unwrap(),
            regex::Regex::new(r"(?i)\.insert\s*\([^)]*password\s*:").unwrap(),
            regex::Regex::new(r"(?i)INSERT\s+INTO\s+.*password").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static PASSWORD_HASH_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"(?i)bcrypt").unwrap(),
            regex::Regex::new(r"(?i)argon2").unwrap(),
            regex::Regex::new(r"(?i)scrypt").unwrap(),
            regex::Regex::new(r"(?i)pbkdf2").unwrap(),
            regex::Regex::new(r"(?i)hashPassword").unwrap(),
            regex::Regex::new(r"(?i)hash_password").unwrap(),
            regex::Regex::new(r"(?i)password_hash").unwrap(),
            regex::Regex::new(r"(?i)passwordHash").unwrap(),
            // `hashed`-prefix variable form: `password: hashedPassword` is a safe
            // write of an already-hashed value (papermark stores share-link
            // passwords this way), not plaintext.
            regex::Regex::new(r"(?i)hashed[_]?password").unwrap(),
            regex::Regex::new(r"(?i)\.hash\s*\(").unwrap(),
            regex::Regex::new(r"(?i)hashSync\s*\(").unwrap(),
            regex::Regex::new(r"(?i)make_password\s*\(").unwrap(),
            regex::Regex::new(r"(?i)generate_password_hash\s*\(").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static NPLUS1_ORM_IN_LOOP_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            // Bare `.get` and `.query` are intentionally not lookup
            // signals: Map/HashMap reads and query-cache access inside loops
            // are normal in-memory work. Generic names only count when the
            // receiver itself is database-shaped.
            regex::Regex::new(
                r"(?s)for\s*\([^)]*\)\s*\{[^}]{0,200}(?:\.(?:findUnique|findFirst|findOne|findById|findByPk)|\b(?:db|database|repository|repo)\.(?:get|query))\s*\(",
            )
            .unwrap(),
            regex::Regex::new(
                r"(?s)\.forEach\s*\([^)]*=>\s*\{[^}]{0,200}(?:\.(?:findUnique|findFirst|findOne|findById|findByPk)|\b(?:db|database|repository|repo)\.(?:get|query))\s*\(",
            )
            .unwrap(),
            regex::Regex::new(
                r"(?s)\.map\s*\([^)]*=>\s*\{[^}]{0,200}(?:\.(?:findUnique|findFirst|findOne|findById|findByPk)|\b(?:db|database|repository|repo)\.(?:get|query))\s*\(",
            )
            .unwrap(),
            regex::Regex::new(
                r"(?s)for\s+\w+\s+in\s+[^:]+:[^}]{0,200}(?:\b[A-Za-z_][A-Za-z0-9_]*\.objects\.(?:get|filter)|\b(?:session|db)\.query)\s*\(",
            )
            .unwrap(),
        ]
    });

#[cfg(test)]
mod tests {
    use super::{
        HARDCODED_SECRET_PATTERNS, PASSWORD_HASH_PATTERNS, PASSWORD_STORE_PATTERNS,
        PLACEHOLDER_PATTERNS,
    };

    #[test]
    fn placeholder_markers_require_word_boundaries() {
        let matches = |source: &str| {
            PLACEHOLDER_PATTERNS
                .iter()
                .any(|pattern| pattern.is_match(source))
        };
        // Real markers still count, in any case.
        assert!(matches("// TODO: wire up billing"));
        assert!(matches("# fixme later"));
        assert!(matches("const HACK = true; // hack"));
        assert!(matches("// XXX revisit"));
        assert!(!matches("the hackathon submission page"));
        assert!(!matches("size: 'XXXL'"));
        assert!(!matches("const toDoList = [];"));
        assert!(!matches("shack placeholders_none hacker"));
    }

    fn matches_secret(source: &str) -> bool {
        HARDCODED_SECRET_PATTERNS
            .iter()
            .any(|pattern| pattern.is_match(source))
    }

    #[test]
    fn pem_private_key_armor_headers_are_secrets() {
        assert!(matches_secret("-----BEGIN PRIVATE KEY-----")); // gitleaks:allow
        assert!(matches_secret("-----BEGIN RSA PRIVATE KEY-----")); // gitleaks:allow
        assert!(matches_secret("-----BEGIN OPENSSH PRIVATE KEY-----")); // gitleaks:allow
        assert!(matches_secret("-----BEGIN ENCRYPTED PRIVATE KEY-----")); // gitleaks:allow
        assert!(matches_secret("-----BEGIN PGP PRIVATE KEY BLOCK-----")); // gitleaks:allow

        // The \n-joined private_key field of a committed service-account JSON.
        assert!(matches_secret(
            r#""private_key": "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBg""#
        ));
        // Public halves and certificates are fine to commit.
        assert!(!matches_secret("-----BEGIN PUBLIC KEY-----"));
        assert!(!matches_secret("-----BEGIN CERTIFICATE-----"));
        assert!(!matches_secret("-----BEGIN RSA PUBLIC KEY-----"));
    }

    #[test]
    fn expanded_provider_key_formats_are_secrets() {
        // Formats the web-side exposed_keys check knew but the source path
        // did not.
        assert!(matches_secret(
            r#"const key = "AIzaABCDEFGHIJKLMNOPQRSTUVWXYZ123456789";"# // gitleaks:allow
        ));
        assert!(matches_secret(
            r#"token: "npm_abcdefghijklmnopqrstuvwxyz0123456789""# // gitleaks:allow
        ));
        assert!(matches_secret(
            r#"stripe = "rk_live_abcdefghijklmnopqrstuvwxyz""# // gitleaks:allow
        ));
        assert!(matches_secret(
            r#"apiKey: "sk-proj-abc123_DEF456-ghi789jkl012""# // gitleaks:allow
        ));
        assert!(matches_secret(
            r#"pat = "github_pat_11ABCDEFGHIJKLMNOPQRSTUV""#
        ));
        // Shape near-misses stay quiet.
        assert!(!matches_secret(r#"const short = "AIzaShort";"#));
        assert!(!matches_secret(r#"registry: "npm_config_registry""#));
        assert!(!matches_secret(r#"const label = "sk-proj-short";"#));
    }

    fn matches_store(source: &str) -> bool {
        PASSWORD_STORE_PATTERNS
            .iter()
            .any(|pattern| pattern.is_match(source))
    }

    fn matches_hash(source: &str) -> bool {
        PASSWORD_HASH_PATTERNS
            .iter()
            .any(|pattern| pattern.is_match(source))
    }

    #[test]
    fn hashed_password_variable_counts_as_hashing() {
        assert!(matches_hash("const hashedPassword = await hashLink(pw);"));
        assert!(matches_hash("data: { password: hashed_password }"));
        // Existing hashing signals still recognised.
        assert!(matches_hash("const pw = await bcrypt.hash(input, 10);"));
        assert!(matches_hash("passwordHash: await argon2.hash(pw)"));
        // A bare plaintext password field is NOT hashing.
        assert!(!matches_hash("data: { password: req.body.password }"));
    }

    #[test]
    fn password_store_matches_real_db_writes() {
        assert!(matches_store(
            "await db.user.create({ data: { email, password: hashed } })"
        ));
        assert!(matches_store(
            "knex('users').insert({ password: hashed, email })"
        ));
        assert!(matches_store(
            "INSERT INTO users (email, password) VALUES ($1, $2)"
        ));
    }

    #[test]
    fn password_store_ignores_client_form_value_objects() {
        assert!(!matches_store(
            "defaultValues: { email: \"\", password: \"\" }"
        ));
        assert!(!matches_store(
            "const payload = { email, password: data.password };"
        ));
        assert!(!matches_store(
            "signIn({ email, password: input.password })"
        ));
    }
}
