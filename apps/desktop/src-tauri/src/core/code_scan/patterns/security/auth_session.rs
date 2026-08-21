use std::sync::LazyLock;

pub(in crate::core::code_scan) static CORS_WILDCARD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)access-control-allow-origin[^\n]*\*"#).unwrap(),
            regex::Regex::new(r#"(?i)origin\s*:\s*["']\*["']"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static CORS_CREDENTIAL_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)access-control-allow-credentials[^\n]*true"#).unwrap(),
            regex::Regex::new(r#"(?i)credentials\s*:\s*true"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static CORS_RUNTIME_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(r#"(?i)\b(?:set|append|insert|header|set_header)\b[^\n]*(?:access-control-allow-origin|access-control-allow-credentials)"#).unwrap(),
        regex::Regex::new(r#"(?i)\ballow_origin\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)\ballow_credentials\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)\bcors\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)headers\s*:\s*\{"#).unwrap(),
    ]
    });

/// Direct reflection of a request Origin into `Access-Control-Allow-Origin`.
/// Validated allowlist flows through intermediate variables do not match.
pub(in crate::core::code_scan) static CORS_ORIGIN_REFLECTION_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            // One-line reflection forms across supported server frameworks.
            regex::Regex::new(
                r#"(?i)access-control-allow-origin["']?\s*\]?\s*[,:=]\s*[^,;\n]{0,40}?(?:req(?:uest)?\.headers\.origin\b|req(?:uest)?\.headers\.get\(\s*["']origin["']\s*\)|req(?:uest)?\.headers\[\s*["']origin["']\s*\]|\$_SERVER\[\s*["']HTTP_ORIGIN["']\s*\])"#,
            )
            .expect("static CORS reflection regex"), // allow-expect: compile-time literal regex
            // Middleware shorthand that reflects every request origin.
            regex::Regex::new(r"\borigin\s*:\s*true\b")
                .expect("static CORS origin-true regex"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static AUTH_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(
    || {
        vec![
            regex::Regex::new(r"\bgetServerSession\b").unwrap(),
            regex::Regex::new(r"\bcurrentUser\b").unwrap(),
            regex::Regex::new(r"\brequireAuth\b").unwrap(),
            regex::Regex::new(r"\brequireUser\b").unwrap(),
            regex::Regex::new(r"\bauth\s*\(").unwrap(),
            regex::Regex::new(r"\b(?:req|request|ctx)\.session\b").unwrap(),
            regex::Regex::new(r"\bsession-token\b").unwrap(),
            regex::Regex::new(r"\bverify(Auth|Token|Jwt|JWT)\b").unwrap(),
            regex::Regex::new(r"\bclerk\b").unwrap(),
            regex::Regex::new(r"\bsupabase\.auth\b").unwrap(),
            regex::Regex::new(r"\bctx\.user\b").unwrap(),
            regex::Regex::new(r"\buserId\b").unwrap(),
            regex::Regex::new(r"\bensureAuthenticated\b").unwrap(),
            // Laravel and WordPress identity gates.
            regex::Regex::new(r"\bAuth::(?:check|user|id|guard|guest|attempt|login)").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            // Match chain middleware with scalar or array arguments.
            regex::Regex::new(r#"(?:->|::)middleware\s*\(\s*\[?\s*['"]auth"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            // Keep group middleware matches inside their config arrays.
            regex::Regex::new(r#"['"]middleware['"]\s*=>\s*\[?[^;\]]{0,80}['"]auth"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bis_user_logged_in\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bwp_get_current_user\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bget_current_user_id\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bauth_redirect\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"\badd_(?:menu|submenu|options|management|theme|plugins|users|dashboard)_page\s*\(",
            )
            .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            // WordPress capability checks also imply authentication.
            regex::Regex::new(r"\b(?:current_)?user_can\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    },
);

pub(in crate::core::code_scan) static AUTH_CLIENT_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\buseSession\b").unwrap(),
            regex::Regex::new(r"\buseUser\b").unwrap(),
            regex::Regex::new(r"\buseAuth\b").unwrap(),
            regex::Regex::new(r"\bSignedIn\b").unwrap(),
            regex::Regex::new(r"\bSignedOut\b").unwrap(),
            regex::Regex::new(r"\bonAuthStateChanged\b").unwrap(),
            regex::Regex::new(r"\bSessionProvider\b").unwrap(),
            regex::Regex::new(r"\bClerkProvider\b").unwrap(),
            regex::Regex::new(r"\bsupabase\.auth\.(getUser|getSession|signIn|signOut)").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static NEXT_MIDDLEWARE_MATCHER_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| vec![regex::Regex::new(r#"["'](/[^"']+)["']"#).unwrap()]);

pub(in crate::core::code_scan) static AUTHZ_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(
    || {
        vec![
            regex::Regex::new(r"\brequireRole\b").unwrap(),
            regex::Regex::new(r"\brequirePermission\b").unwrap(),
            regex::Regex::new(r"\bhasRole\b").unwrap(),
            regex::Regex::new(r"\bhasPermission\b").unwrap(),
            regex::Regex::new(r"\bcan\s*\(").unwrap(),
            regex::Regex::new(r"\bcanAccess\b").unwrap(),
            regex::Regex::new(r"\bauthorize\s*\(").unwrap(),
            regex::Regex::new(r"\bpolicy\b").unwrap(),
            regex::Regex::new(r"\bisAdmin\b").unwrap(),
            regex::Regex::new(r#"\brole\s*===\s*['"]admin['"]"#).unwrap(),
            regex::Regex::new(r"\bpermission").unwrap(),
            regex::Regex::new(r"\bability\.can\b").unwrap(),
            // PHP: WordPress capability checks, the Laravel Gate facade
            // (`authorize(` and `can(` above already match the arrow-call
            // forms), and capability-carrying WP admin-page registration.
            regex::Regex::new(r"\bcurrent_user_can\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\buser_can\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bGate::(?:allows|denies|authorize|check|any|none|inspect)")
                .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"\badd_(?:menu|submenu|options|management|theme|plugins|users|dashboard)_page\s*\(",
            )
            .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    },
);

pub(in crate::core::code_scan) static REQUEST_TOKEN_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(r#"(?i)authorization"#).unwrap(),
        regex::Regex::new(r#"(?i)\bbearer\b"#).unwrap(),
        regex::Regex::new(r#"(?is)searchParams\.get\(\s*["'`](?:token|jwt|access_token|id_token|refresh_token)["'`]\s*\)"#).unwrap(),
        regex::Regex::new(r#"(?is)(?:formData|data|input|values)\.get\s*\(\s*["'`](?:token|jwt|access_token|id_token|refresh_token)["'`]\s*\)"#).unwrap(),
        regex::Regex::new(r#"(?is)(?:body|input|payload|params|query|data)\.(?:token|jwt|accessToken|idToken|refreshToken)\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bcookies?\s*\(.*(?:token|jwt)"#).unwrap(),
    ]
    });

pub(in crate::core::code_scan) static JWT_DECODE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(r"\bjwt\.decode\s*\(").unwrap(),
        regex::Regex::new(r"\bdecodeJwt\s*\(").unwrap(),
        regex::Regex::new(r"\bjwtDecode\s*\(").unwrap(),
        // Bound decoder-to-payload distance to avoid combining unrelated calls.
        regex::Regex::new(r#"(?is)\batob\s*\(.{0,120}?split\s*\(\s*["'`]\.["'`]\s*\)\s*\[\s*1\s*\]"#).unwrap(),
        regex::Regex::new(r#"(?is)\bBuffer\.from\s*\(.{0,120}?split\s*\(\s*["'`]\.["'`]\s*\)\s*\[\s*1\s*\]\s*,\s*["'`](?:base64|base64url)["'`]"#).unwrap(),
    ]
    });

pub(in crate::core::code_scan) static JWT_VERIFY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bjwt\.verify\s*\(").unwrap(),
            regex::Regex::new(r"\bjwtVerify\s*\(").unwrap(),
            regex::Regex::new(r"\bverifyJwt\s*\(").unwrap(),
            regex::Regex::new(r"\bverifyToken\s*\(").unwrap(),
            regex::Regex::new(r"\bdecodeAndVerify\b").unwrap(),
            regex::Regex::new(r"\bgetToken\s*\(").unwrap(),
            regex::Regex::new(r"\bverifyAuthToken\b").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static OAUTH_CODE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(r#"(?is)searchParams\.get\(\s*["'`]code["'`]\s*\)"#).unwrap(),
        regex::Regex::new(r#"(?is)(?:body|input|payload|params|query|data)\.(?:code|authCode|authorizationCode)\b"#).unwrap(),
        regex::Regex::new(r#"(?is)(?:formData|data|input|values)\.get\s*\(\s*["'`](?:code|authCode|authorizationCode)["'`]\s*\)"#).unwrap(),
        regex::Regex::new(r#"(?is)\breq\.(?:query|body)\.(?:code|authCode|authorizationCode)\b"#).unwrap(),
    ]
    });

pub(in crate::core::code_scan) static OAUTH_TOKEN_EXCHANGE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?is)grant_type["'` ]*[:=]["'` ]*authorization_code"#).unwrap(),
            regex::Regex::new(r#"(?is)authorizationCodeGrant\s*\("#).unwrap(),
            regex::Regex::new(r#"(?is)exchangeCodeForTokens?\s*\("#).unwrap(),
            regex::Regex::new(r#"(?is)oauth2\.googleapis\.com/token"#).unwrap(),
            regex::Regex::new(r#"(?is)login\.github\.com/login/oauth/access_token"#).unwrap(),
            regex::Regex::new(r#"(?is)/oauth/token"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static OAUTH_STATE_GUARD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?is)searchParams\.get\(\s*["'`]state["'`]\s*\)"#).unwrap(),
            regex::Regex::new(r#"(?is)(?:body|input|payload|params|query|data)\.state\b"#).unwrap(),
            regex::Regex::new(
                r#"(?is)(?:formData|data|input|values)\.get\s*\(\s*["'`]state["'`]\s*\)"#,
            )
            .unwrap(),
            regex::Regex::new(r#"\boauth_state\b"#).unwrap(),
            regex::Regex::new(r#"\bexpectedState\b"#).unwrap(),
            regex::Regex::new(r#"\bstoredState\b"#).unwrap(),
            regex::Regex::new(r#"\bstate\s*!==\s*"#).unwrap(),
            regex::Regex::new(r#"\bstate\s*===\s*"#).unwrap(),
            regex::Regex::new(r#"\btimingSafeEqual\s*\("#).unwrap(),
            regex::Regex::new(
                r#"\bcookies\s*\(\)\.get\s*\(\s*["'`](?:oauth_state|state)["'`]\s*\)"#,
            )
            .unwrap(),
            regex::Regex::new(
                r#"\brequest\.cookies\.get\s*\(\s*["'`](?:oauth_state|state)["'`]\s*\)"#,
            )
            .unwrap(),
        ]
    });

pub(in crate::core::code_scan) static OAUTH_PKCE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?is)code_verifier"#).unwrap(),
            regex::Regex::new(r#"\bcodeVerifier\b"#).unwrap(),
            regex::Regex::new(r#"\bpkce\b"#).unwrap(),
            regex::Regex::new(r#"\bS256\b"#).unwrap(),
            regex::Regex::new(r#"\bgenerateCodeVerifier\b"#).unwrap(),
            regex::Regex::new(r#"\bcreateCodeVerifier\b"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static OAUTH_CLIENT_SECRET_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?is)client_secret"#).unwrap(),
            regex::Regex::new(r#"\bCLIENT_SECRET\b"#).unwrap(),
            regex::Regex::new(r#"\bclientSecret\b"#).unwrap(),
            regex::Regex::new(r#"\bprivate_key_jwt\b"#).unwrap(),
            regex::Regex::new(r#"\bclientAssertion\b"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static ONE_TIME_TOKEN_FLOW_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)\bpassword[_ -]?reset\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bresetPassword\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bmagic[_ -]?link\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bsignInWithOtp\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bemail[_ -]?verification\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bverify[_ -]?email\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\baccept[_ -]?invite\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\binvite[_ -]?token\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\breset[_ -]?token\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bverification[_ -]?token\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bmagic[_ -]?token\b"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static ONE_TIME_TOKEN_HASH_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bcreateHash\s*\(").unwrap(),
            regex::Regex::new(r"\bsha256\b").unwrap(),
            regex::Regex::new(r"\bsubtle\.digest\s*\(").unwrap(),
            regex::Regex::new(r"\bhashToken\b").unwrap(),
            regex::Regex::new(r"\btokenHash\b").unwrap(),
            regex::Regex::new(r"\bhashedToken\b").unwrap(),
            regex::Regex::new(r"\bverificationTokenHash\b").unwrap(),
            regex::Regex::new(r"\bresetTokenHash\b").unwrap(),
            regex::Regex::new(r"\binviteTokenHash\b").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static ONE_TIME_TOKEN_EXPIRY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)\bexpiresAt\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bexpires_at\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bexpiredAt\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bexpiration\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bisExpired\b"#).unwrap(),
            // Require explicit expiry logic; generic clock calls also appear in
            // token creation and do not prove enforcement.
            regex::Regex::new(r#"(?i)\bexpires\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bexpiry\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bvalidUntil\b"#).unwrap(),
            regex::Regex::new(r#"(?i)\bttl\b"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static ONE_TIME_TOKEN_SINGLE_USE_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?i)\busedAt\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bconsumedAt\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bredeemedAt\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bacceptedAt\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bclaimedAt\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bdelete(?:Many)?\s*\("#).unwrap(),
        regex::Regex::new(
            r#"(?is)\bupdate(?:Many)?\s*\([^)]{0,220}\b(?:usedAt|consumedAt|redeemedAt|acceptedAt|claimedAt|tokenUsedAt)\b"#,
        )
        .unwrap(),
        regex::Regex::new(r#"(?i)\binvalidate\w*\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)\bconsume\w*\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)\bredeem\w*\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)\bmark\w*used\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\btokenUsedAt\b"#).unwrap(),
    ]
});

pub(in crate::core::code_scan) static ONE_TIME_TOKEN_RAW_LOOKUP_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(
            r#"(?is)where\s*:\s*\{[^}]{0,220}(?:token|resetToken|inviteToken|verificationToken)\s*:\s*(?:token|body\.[A-Za-z0-9_]+|input\.[A-Za-z0-9_]+|payload\.[A-Za-z0-9_]+|params\.[A-Za-z0-9_]+|query\.[A-Za-z0-9_]+|data\.[A-Za-z0-9_]+)"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)where\s*:\s*\{[^}]{0,220}(?:token|resetToken|inviteToken|verificationToken)\s*:\s*(?:searchParams\.get\(|(?:formData|data|input|values)\.get\()"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)(?:token|resetToken|inviteToken|verificationToken)\s*=\s*(?:searchParams\.get\(|(?:formData|data|input|values)\.get\()"#,
        )
        .unwrap(),
    ]
});

pub(in crate::core::code_scan) static COOKIE_SESSION_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bcookies\s*\(").unwrap(),
            regex::Regex::new(r"\breq\.cookies\b").unwrap(),
            regex::Regex::new(r"\brequest\.cookies\b").unwrap(),
            regex::Regex::new(r"\bset-cookie\b").unwrap(),
            regex::Regex::new(r"session-token").unwrap(),
            regex::Regex::new(r"\bsession\b").unwrap(),
            // PHP: native session machinery and cookie superglobal.
            regex::Regex::new(r"\bsession_start\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\$_SESSION\b").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\$_COOKIE\b").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            // WordPress ajax/admin-post handlers run inside WP's cookie-based
            // auth context even though the file never touches a cookie
            // directly - that context is what CSRF rides.
            regex::Regex::new(r#"['"](?:wp_ajax_|admin_post_)"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static COOKIE_WRITE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"\bcookies\s*\(\)\.set\s*\("#).unwrap(),
            regex::Regex::new(r#"\bresponse\.cookies\.set\s*\("#).unwrap(),
            regex::Regex::new(r#"\bNextResponse\.[A-Za-z_]+\s*\([^)]*\)\.cookies\.set\s*\("#)
                .unwrap(),
            regex::Regex::new(r#"\bsetHeader\s*\(\s*["'`]Set-Cookie["'`]"#).unwrap(),
            regex::Regex::new(r#"\bappend\s*\(\s*["'`]Set-Cookie["'`]"#).unwrap(),
            regex::Regex::new(r#"\bheaders\s*:\s*\{[^}]{0,240}["'`]Set-Cookie["'`]"#).unwrap(),
            // Require a literal cookie name so PHP object serialization cannot match.
            regex::Regex::new(r#"\bserialize\s*\(\s*["'`]"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"\bcookie\.serialize\s*\("#).unwrap(),
            // PHP: direct cookie writes.
            regex::Regex::new(r"\bsetcookie\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\bsetrawcookie\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static SESSION_COOKIE_NAME_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(r#"(?i)\bnext-auth\.session-token\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bsession-token\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bauth-token\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\brefresh-token\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\baccess-token\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bsession[_-]?id\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\brefresh_token\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\baccess_token\b"#).unwrap(),
        regex::Regex::new(r#"(?i)\bphpsessid\b"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r#"(?i)\bcookies\s*\(\)\.set\s*\(\s*(?:\{[^}]{0,220}\bname\s*:\s*["'`](?:next-auth\.session-token|session-token|session|auth|auth-token|refresh-token|access-token|refresh_token|access_token)["'`]|["'`](?:next-auth\.session-token|session-token|session|auth|auth-token|refresh-token|access-token|refresh_token|access_token)["'`])"#).unwrap(),
        regex::Regex::new(r#"(?i)\bresponse\.cookies\.set\s*\(\s*(?:\{[^}]{0,220}\bname\s*:\s*["'`](?:next-auth\.session-token|session-token|session|auth|auth-token|refresh-token|access-token|refresh_token|access_token)["'`]|["'`](?:next-auth\.session-token|session-token|session|auth|auth-token|refresh-token|access-token|refresh_token|access_token)["'`])"#).unwrap(),
        regex::Regex::new(r#"(?i)\bserialize\s*\(\s*["'`](?:next-auth\.session-token|session-token|session|auth|auth-token|refresh-token|access-token|refresh_token|access_token)["'`]"#).unwrap(),
    ]
    });

// Match structural HttpOnly configuration, not prose mentioning the word.
pub(in crate::core::code_scan) static COOKIE_HTTP_ONLY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)\bhttpOnly\s*:\s*true"#).unwrap(),
            // HttpOnly as a Set-Cookie attribute (literal `; HttpOnly`)
            regex::Regex::new(r#"(?i)[;,]\s*httpOnly\b"#).expect("static httpOnly attr regex"), // allow-expect: compile-time literal regex
            // Express/Fastify-style boolean prop in a cookie-options object
            regex::Regex::new(r#"(?i)\bhttpOnly\s*=\s*true\b"#)
                .expect("static httpOnly prop regex"), // allow-expect: compile-time literal regex
            // PHP: setcookie options array, Laravel config/session.php,
            // and the ini directive.
            regex::Regex::new(r#"(?i)['"]httponly['"]\s*=>\s*true"#)
                .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?i)['"]http_only['"]\s*=>\s*true"#)
                .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"session\.cookie_httponly").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            // Positional 7-arg setcookie: a `..., true, true)` tail is
            // (secure, httponly). A lone trailing `true` is ambiguous (it can
            // be the 6-arg secure form), so only the unambiguous pair counts.
            regex::Regex::new(r"(?s)\bsetcookie\s*\([^;]{0,240}\btrue\s*,\s*true\s*\)")
                .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

// Match structural cookie options; generic "secure" text is not evidence.
pub(in crate::core::code_scan) static COOKIE_SECURE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)\bsecure\s*:\s*true"#).unwrap(),
            regex::Regex::new(
                r#"(?i)\bsecure\s*:\s*process\.env\.node_env\s*===?\s*["'`]production["'`]"#,
            )
            .unwrap(),
            regex::Regex::new(r#"(?i)\bsecure\s*:\s*!?(?:isProd|isProduction)"#).unwrap(),
            // Secure as a Set-Cookie attribute (literal `; Secure`)
            regex::Regex::new(r#"(?i)[;,]\s*secure\b\s*(?:[;,]|$|[\\\\"'])"#)
                .expect("static secure attr regex"), // allow-expect: compile-time literal regex
            // Express/Fastify-style boolean prop
            regex::Regex::new(r#"(?i)\bsecure\s*=\s*true\b"#).expect("static secure prop regex"), // allow-expect: compile-time literal regex
            // __Host- prefix is a stronger signal: spec requires Secure
            regex::Regex::new(r#"["'`]__Host-"#).expect("static __Host- prefix regex"), // allow-expect: compile-time literal regex
            // PHP: setcookie options array, Laravel config/session.php, the
            // ini directive, and the unambiguous positional (secure, httponly)
            // pair - see the httpOnly list for why a lone `true` doesn't count.
            regex::Regex::new(r#"(?i)['"]secure['"]\s*=>\s*true"#)
                .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"session\.cookie_secure").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?s)\bsetcookie\s*\([^;]{0,240}\btrue\s*,\s*true\s*\)")
                .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static COOKIE_SAMESITE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r#"(?i)\bsameSite\s*:\s*["'`](?:lax|strict|none)["'`]"#).unwrap(),
            regex::Regex::new(r#"(?i)\bsamesite\s*=\s*(?:lax|strict|none)"#).unwrap(),
            // PHP: setcookie options array / Laravel config, and the ini
            // directive. (SameSite has no positional setcookie form.)
            regex::Regex::new(
                r#"(?i)['"](?:samesite|same_site)['"]\s*=>\s*['"](?:lax|strict|none)['"]"#,
            )
            .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"session\.cookie_samesite").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

// Require structural CSRF controls; generic origin, referrer, or SameSite text is insufficient.
pub(in crate::core::code_scan) static CSRF_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(
    || {
        vec![
            // Case-insensitive: the acronym is normally written "CSRF"
            // (comments, X-CSRF-Token headers); lowercase-only missed it.
            regex::Regex::new(r"(?i)\bcsrf\b").expect("static csrf regex"), // allow-expect: compile-time literal regex
            // Compound identifiers (csrfToken, csrf_token) have no word
            // boundary after "csrf", so the bounded pattern can't see them.
            regex::Regex::new(r"(?i)csrf[-_]?token").unwrap(),
            regex::Regex::new(r"csurf").unwrap(),
            regex::Regex::new(r"authenticity[_-]?token").unwrap(),
            regex::Regex::new(r"anti[-_ ]?forgery").unwrap(),
            regex::Regex::new(r"\bdouble[-_ ]?submit\b").unwrap(),
            // SameSite as a cookie attribute (strict|lax|none) - not the
            // bare word, which matched copy and comments.
            regex::Regex::new(r#"(?i)sameSite\s*:\s*["'`](?:lax|strict|none)["'`]"#).unwrap(),
            regex::Regex::new(r#"(?i)samesite\s*=\s*(?:lax|strict|none)"#).unwrap(),
            regex::Regex::new(
                r#"(?i)['"](?:samesite|same_site)['"]\s*=>\s*['"](?:lax|strict|none)['"]"#,
            )
            .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            // WordPress nonces are its CSRF mechanism; the Laravel forms
            // (@csrf, csrf_token, VerifyCsrfToken) are already covered by the
            // csrf patterns above.
            regex::Regex::new(
                r"\b(?:wp_verify_nonce|check_admin_referer|check_ajax_referer|wp_nonce_field|wp_create_nonce)\s*\(",
            )
            .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            // Explicit origin/referer header VALIDATION, not bare token.
            regex::Regex::new(r#"(?i)\borigin\b\s*[!=]==?"#).unwrap(),
            regex::Regex::new(r#"(?i)\breferer\b\s*[!=]==?"#).unwrap(),
            // A call to an origin-validation helper (isSameOrigin,
            // verifyOrigin, checkRequestOrigin,...): the comparison itself
            // often lives in a shared module, so the call site is the only
            // in-file evidence of the guard.
            regex::Regex::new(
                r"(?i)\b(?:is|verify|check|validate|assert|ensure|require|same)[a-z0-9_]*origin[a-z0-9_]*\s*\(",
            )
            .unwrap(),
            regex::Regex::new(r"verifyCsrf").unwrap(),
        ]
    },
);
