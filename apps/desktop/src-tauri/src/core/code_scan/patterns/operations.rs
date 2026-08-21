use std::sync::LazyLock;

pub(in crate::core::code_scan) static ENV_USAGE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"process\.env").unwrap(),
            regex::Regex::new(r"import\.meta\.env").unwrap(),
            regex::Regex::new(r"Deno\.env\.get").unwrap(),
            regex::Regex::new(r"std::env::var").unwrap(),
            regex::Regex::new(r"os\.getenv").unwrap(),
            regex::Regex::new(r"System\.getenv").unwrap(),
        ]
    });

/// Next.js flags that suppress build-time TypeScript or lint failures.
#[rustfmt::skip] // Keep the allow-expect marker on the call.
pub(in crate::core::code_scan) static NEXTCONFIG_IGNORE_BUILD_ERRORS_PATTERN: LazyLock<
    regex::Regex,
> = LazyLock::new(|| {
    regex::Regex::new(r"ignoreBuildErrors\s*:\s*true")
        .expect("static ignoreBuildErrors regex") // allow-expect: compile-time literal regex
});

pub(in crate::core::code_scan) static NEXTCONFIG_IGNORE_LINT_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(r"ignoreDuringBuilds\s*:\s*true")
            .expect("static ignoreDuringBuilds regex") // allow-expect: compile-time literal regex
    });

pub(in crate::core::code_scan) static FEATURE_FLAG_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"feature[-_ ]?flag").unwrap(),
            regex::Regex::new(r"isFeatureEnabled").unwrap(),
            regex::Regex::new(r"launchdarkly").unwrap(),
            regex::Regex::new(r"flagsmith").unwrap(),
            regex::Regex::new(r"posthog\.(isFeatureEnabled|getFeatureFlag)").unwrap(),
            regex::Regex::new(r"ENABLE_AI").unwrap(),
            regex::Regex::new(r"DISABLE_AI").unwrap(),
            regex::Regex::new(r"AI_ENABLED").unwrap(),
            regex::Regex::new(r"kill[-_ ]?switch").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static STRUCTURED_LOGGING_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bpino\b").unwrap(),
            regex::Regex::new(r"\bwinston\b").unwrap(),
            regex::Regex::new(r"\bbunyan\b").unwrap(),
            regex::Regex::new(r"createLogger").unwrap(),
            regex::Regex::new(r"\blogger\.").unwrap(),
            regex::Regex::new(r"\btracing::").unwrap(),
            regex::Regex::new(r"\bslog::").unwrap(),
            regex::Regex::new(r"\blogrus\b").unwrap(),
            regex::Regex::new(r"console\.error\s*\(\s*\{").unwrap(),
            regex::Regex::new(
                r"(?s)\bfunction\s+logEvent\b.{0,1000}JSON\.stringify\s*\(\s*\{[^}]*\bevent\b",
            )
            .expect("structured logging regex is valid"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static ERROR_REPORTING_PACKAGES: &[&str] = &[
    "@sentry/nextjs",
    "@sentry/node",
    "@sentry/react",
    "@sentry/browser",
    "@bugsnag/js",
    "@bugsnag/node",
    "bugsnag",
    "rollbar",
    "honeybadger",
    "airbrake",
];

pub(in crate::core::code_scan) static STRUCTURED_LOGGING_PACKAGES: &[&str] = &[
    "pino", "winston", "bunyan", "logrus", "zerolog", "tracing", "slog",
];

pub(in crate::core::code_scan) static AI_OBSERVABILITY_PACKAGES: &[&str] = &[
    "langfuse",
    "langsmith",
    "helicone",
    "@helicone/helpers",
    "braintrust",
    "humanloop",
    "portkey-ai",
    "@traceloop/node-server-sdk",
    "@opentelemetry/api",
    "@opentelemetry/sdk-node",
];

pub(in crate::core::code_scan) static FRONTEND_APP_PACKAGES: &[&str] = &[
    "react",
    "react-dom",
    "next",
    "@remix-run/react",
    "react-router-dom",
];

pub(in crate::core::code_scan) static DATABASE_PACKAGES: &[&str] = &[
    "@prisma/client",
    "prisma",
    "drizzle-orm",
    "knex",
    "sequelize",
    "sqlx",
    "postgres",
    "pg",
    "mysql2",
    "typeorm",
    "supabase",
];

pub(in crate::core::code_scan) static ERROR_BOUNDARY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bErrorBoundary\b").unwrap(),
            regex::Regex::new(r"\bcomponentDidCatch\b").unwrap(),
            regex::Regex::new(r"\bgetDerivedStateFromError\b").unwrap(),
            regex::Regex::new(r"\berrorElement\b").unwrap(),
            regex::Regex::new(r"\buseErrorBoundary\b").unwrap(),
            regex::Regex::new(r"\bglobal-error\b").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static BACKGROUND_JOB_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bbullmq\b").unwrap(),
            regex::Regex::new(r"\bbull\b").unwrap(),
            regex::Regex::new(r"\bpg[-_ ]?boss\b").unwrap(),
            regex::Regex::new(r"\bagenda\b").unwrap(),
            regex::Regex::new(r"\bbree\b").unwrap(),
            regex::Regex::new(r"\bnode-cron\b").unwrap(),
            regex::Regex::new(r"\bcron\.schedule\s*\(").unwrap(),
            regex::Regex::new(r"\bnew Worker\s*\(").unwrap(),
            regex::Regex::new(r"\bnew Queue\s*\(").unwrap(),
            regex::Regex::new(r"\bserve\(\s*inngest\b").unwrap(),
            regex::Regex::new(r"\btrigger\.dev\b").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static JOB_VISIBILITY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\bbull-board\b").unwrap(),
            regex::Regex::new(r"\barena\b").unwrap(),
            regex::Regex::new(r"\bqueueEvents\b").unwrap(),
            regex::Regex::new(r"\bworker\.on\s*\(").unwrap(),
            regex::Regex::new(r"\bjob\.progress\s*\(").unwrap(),
            regex::Regex::new(r"/jobs").unwrap(),
            regex::Regex::new(r"/queues").unwrap(),
            regex::Regex::new(r"\bqueue status\b").unwrap(),
            regex::Regex::new(r"\bjob status\b").unwrap(),
            regex::Regex::new(r"\bjob dashboard\b").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static LINTER_CONFIG_FILES: &[&str] = &[
    ".eslintrc",
    ".eslintrc.js",
    ".eslintrc.cjs",
    ".eslintrc.json",
    ".eslintrc.yml",
    ".eslintrc.yaml",
    "eslint.config.js",
    "eslint.config.mjs",
    "eslint.config.cjs",
    "eslint.config.ts",
    "biome.json",
    "biome.jsonc",
    ".prettierrc",
    ".prettierrc.js",
    ".prettierrc.cjs",
    ".prettierrc.json",
    ".prettierrc.yml",
    ".prettierrc.yaml",
    ".prettierrc.toml",
    "prettier.config.js",
    "prettier.config.cjs",
    "prettier.config.ts",
    "deno.json",
    "deno.jsonc",
    ".oxlintrc.json",
    "oxlintrc.json",
    ".ruff.toml",
    "ruff.toml",
    "pyproject.toml",
    ".flake8",
    "setup.cfg",
    "clippy.toml",
    ".clippy.toml",
];

pub(in crate::core::code_scan) static TEST_CONFIG_FILES: &[&str] = &[
    "jest.config.js",
    "jest.config.cjs",
    "jest.config.mjs",
    "jest.config.ts",
    "vitest.config.ts",
    "vitest.config.js",
    "vitest.config.mjs",
    "vitest.config.mts",
    "cypress.config.ts",
    "cypress.config.js",
    "cypress.config.cjs",
    "playwright.config.ts",
    "playwright.config.js",
    "pytest.ini",
    "pyproject.toml",
    "conftest.py",
    "setup.cfg",
    ".mocharc.yml",
    ".mocharc.json",
    "karma.conf.js",
    "ava.config.js",
    "ava.config.cjs",
    "ava.config.mjs",
];

pub(in crate::core::code_scan) static DUPLICATE_UTILITY_GROUPS: &[(&[&str], &str)] = &[
    (
        &["axios", "node-fetch", "got", "ky", "undici"],
        "HTTP client",
    ),
    (&["moment", "dayjs", "date-fns", "luxon"], "date/time"),
    (&["lodash", "underscore", "ramda"], "utility"),
    (&["uuid", "nanoid", "cuid", "ulid"], "ID generation"),
    (
        &["chalk", "picocolors", "kleur", "colorette", "ansi-colors"],
        "terminal color",
    ),
    (&["dotenv", "envalid", "env-var"], "env loading"),
    (
        &["commander", "yargs", "meow", "cac", "citty"],
        "CLI argument parsing",
    ),
    (&["winston", "pino", "bunyan", "log4js"], "logging"),
    (
        &[
            "express-validator",
            "joi",
            "yup",
            "zod",
            "valibot",
            "superstruct",
        ],
        "validation",
    ),
];
