use std::sync::LazyLock;

pub(in crate::core::code_scan) static SERVER_DB_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        // Require imports, constructors, calls, or SQL rather than bare tool names.
        // `@prisma/client` alone is excluded because it also exports browser-safe types.
        vec![
            regex::Regex::new(
                r#"(?:\bPrismaClient\b|\bprisma\.|/prisma["']|drizzle-orm|\bdrizzle\s*\(|["']knex["']|\bknex\s*\(|["']sequelize["']|new\s+Sequelize\b|\bsqlx::|\bpostgres\s*\(|["']postgres["']|["']mysql2?(?:/|["'])|\bmysql\s*\.|\bpg\.\b|SELECT\s+.+\s+FROM)"#,
            )
            .unwrap(),
            // PHP usage shapes, same discipline as above (calls, not bare
            // words): the WordPress $wpdb handle, the Laravel DB facade, and
            // direct PDO construction.
            regex::Regex::new(
                r"(?:\$wpdb->|\bDB::(?:table|select|insert|update|delete|statement|transaction|raw)\b|new\s+PDO\b)",
            )
            .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

/// Database access shapes, including Supabase's browser-safe Data API.
pub(in crate::core::code_scan) static DB_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        let mut patterns = SERVER_DB_PATTERNS.iter().cloned().collect::<Vec<_>>();
        patterns.push(
            regex::Regex::new(r"supabase\.from\s*\(").expect("static Supabase data access regex"), // allow-expect: compile-time literal regex
        );
        patterns
    });

pub(in crate::core::code_scan) static DB_LOOKUP_FIELD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(r"\b(user_id|org_id|organization_id|team_id|workspace_id|project_id|tenant_id|account_id|customer_id|owner_id)\b").unwrap(),
        regex::Regex::new(r"\b(userId|orgId|organizationId|teamId|workspaceId|projectId|tenantId|accountId|customerId|ownerId)\b").unwrap(),
    ]
    });

pub(in crate::core::code_scan) static DB_IDENTITY_FIELD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(r"\b(email|slug|username|handle)\b").unwrap(),
        regex::Regex::new(r"\b(external_id|provider_id|public_id|api_key|access_token|invite_token|reset_token|verification_token)\b").unwrap(),
        regex::Regex::new(r"\b(externalId|providerId|publicId|apiKey|accessToken|inviteToken|resetToken|verificationToken)\b").unwrap(),
    ]
    });

pub(in crate::core::code_scan) static DB_INDEX_HINT_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"@@index\s*\(").unwrap(),
            regex::Regex::new(r"@@unique\s*\(").unwrap(),
            regex::Regex::new(r"(?i)\bcreate\s+(?:unique\s+)?index\b").unwrap(),
            regex::Regex::new(r"\buniqueIndex\s*\(").unwrap(),
            regex::Regex::new(r"\bindex\s*\(").unwrap(),
            regex::Regex::new(r"\badd_index\b").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static DB_RLS_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"(?i)enable\s+row\s+level\s+security").unwrap(),
            regex::Regex::new(r"(?i)\bcreate\s+policy\b").unwrap(),
            regex::Regex::new(r"(?i)\bauth\.uid\s*\(").unwrap(),
            regex::Regex::new(r"(?i)\bauth\.role\s*\(").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static SUPABASE_FROM_TABLE_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(r#"(?i)\.from\s*\(\s*["'`]([A-Za-z0-9_.-]+)["'`]\s*\)"#).unwrap()
    });

pub(in crate::core::code_scan) static SUPABASE_TABLE_OPERATION_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(
        r#"(?is)\.from\s*\(\s*["'`]([A-Za-z0-9_.-]+)["'`]\s*\)\s*\.\s*(select|insert|update|upsert|delete)\s*\("#,
    )
    .unwrap()
    });

pub(in crate::core::code_scan) static SQL_RLS_ENABLE_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(
        r#"(?i)alter\s+table(?:\s+if\s+exists)?\s+(?:(?:public|auth)\.)?["`]?([A-Za-z0-9_]+)["`]?\s+enable\s+row\s+level\s+security"#,
    )
    .unwrap()
    });

pub(in crate::core::code_scan) static SQL_CREATE_POLICY_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(
        r#"(?is)create\s+policy\s+.*?\s+on\s+(?:(?:public|auth)\.)?["`]?([A-Za-z0-9_]+)["`]?(.*?);"#,
    )
    .unwrap()
    });

pub(in crate::core::code_scan) static SQL_POLICY_OPERATION_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(r#"(?i)\bfor\s+(all|select|insert|update|delete)\b"#).unwrap()
    });

pub(in crate::core::code_scan) static SQL_POLICY_ROLE_CLAUSE_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(r#"(?is)\bto\s+(.+?)(?:\busing\s*\(|\bwith\s+check\s*\(|$)"#).unwrap()
    });

pub(in crate::core::code_scan) static RLS_AUTH_SCOPING_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"(?i)\bauth\.uid\s*\(").unwrap(),
            regex::Regex::new(r"(?i)\bauth\.role\s*\(").unwrap(),
            regex::Regex::new(r"(?i)\bauth\.jwt\s*\(").unwrap(),
            regex::Regex::new(r#"(?i)current_setting\s*\(\s*['"]request\.jwt\.claims"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static PERMISSIVE_RLS_POLICY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"(?is)\busing\s*\(\s*true\s*\)").unwrap(),
            regex::Regex::new(r"(?is)\bwith\s+check\s*\(\s*true\s*\)").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static SUPABASE_SERVICE_ROLE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"(?i)\bSUPABASE_SERVICE_ROLE(?:_KEY)?\b").unwrap(),
            regex::Regex::new(
                r"(?i)\bNEXT_PUBLIC_[A-Z0-9_]*SUPABASE[A-Z0-9_]*SERVICE[A-Z0-9_]*ROLE",
            )
            .unwrap(),
            regex::Regex::new(r#"(?i)["'`]service_role["'`]"#).unwrap(),
        ]
    });

pub(in crate::core::code_scan) static PRISMA_MODEL_BLOCK_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)model\s+([A-Za-z0-9_]+)\s*\{(.*?)\}").unwrap());

pub(in crate::core::code_scan) static PRISMA_TABLE_MAP_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"@@map\(\s*"([^"]+)"\s*\)"#).unwrap());

pub(in crate::core::code_scan) static PRISMA_FIELD_MAP_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"@map\(\s*"([^"]+)"\s*\)"#).unwrap());

// An optional `schema.` qualifier (`public.users`, `"myschema"."users"`) is
// consumed but not captured, so group 1 is always the table name - not the
// schema. Without this, `create table public.users` captured `public`.
pub(in crate::core::code_scan) static SQL_CREATE_TABLE_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(
            r#"(?i)create\s+table(?:\s+if\s+not\s+exists)?\s+(?:["`]?[A-Za-z0-9_]+["`]?\s*\.\s*)?["`]?([A-Za-z0-9_]+)["`]?"#,
        )
        .unwrap()
    });

pub(in crate::core::code_scan) static SQL_CREATE_TABLE_BLOCK_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(
        r#"(?is)create\s+table(?:\s+if\s+not\s+exists)?\s+(?:["`]?[A-Za-z0-9_]+["`]?\s*\.\s*)?["`]?([A-Za-z0-9_]+)["`]?\s*\((.*?)\)\s*;"#,
    )
    .unwrap()
    });

pub(in crate::core::code_scan) static DRIZZLE_TABLE_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(r#"(?:pgTable|sqliteTable|mysqlTable)\s*\(\s*["']([^"']+)["']"#).unwrap()
    });

pub(in crate::core::code_scan) static DRIZZLE_TABLE_BLOCK_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(
        r#"(?s)(?:pgTable|sqliteTable|mysqlTable)\s*\(\s*["']([^"']+)["']\s*,\s*\{(.*?)\}\s*[,)]"#,
    )
    .unwrap()
    });

pub(in crate::core::code_scan) static DRIZZLE_TABLE_DECL_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| {
        regex::Regex::new(
        r#"(?s)(?:export\s+)?const\s+([A-Za-z0-9_]+)\s*=\s*(?:pgTable|sqliteTable|mysqlTable)\s*\(\s*["']([^"']+)["']\s*,\s*\{(.*?)\}\s*(?:,|\))"#,
    )
    .unwrap()
    });

pub(in crate::core::code_scan) static DB_WRITE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\.create\s*\(").unwrap(),
            regex::Regex::new(r"\.update\s*\(").unwrap(),
            regex::Regex::new(r"\.delete\s*\(").unwrap(),
            regex::Regex::new(r"\.upsert\s*\(").unwrap(),
            regex::Regex::new(r"\.insert\s*\(").unwrap(),
            regex::Regex::new(r"\.insertMany\s*\(").unwrap(),
            regex::Regex::new(r"\.updateMany\s*\(").unwrap(),
            regex::Regex::new(r"\.deleteMany\s*\(").unwrap(),
            regex::Regex::new(r"INSERT\s+INTO").unwrap(),
            regex::Regex::new(r"UPDATE\s+\w+").unwrap(),
            // PHP object calls use `->`, which the dot-based patterns above
            // never see ($wpdb->insert, DB::table(...)->update, Eloquent
            // >save / Model::create).
            regex::Regex::new(r"->(?:insert|update|delete|upsert|save|replace)\s*\(")
                .expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"::create\s*\(").expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

pub(in crate::core::code_scan) static DB_QUERY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\.findUnique(?:OrThrow)?\s*\(").unwrap(),
            regex::Regex::new(r"\.findFirst(?:OrThrow)?\s*\(").unwrap(),
            regex::Regex::new(r"\.findMany\s*\(").unwrap(),
            regex::Regex::new(r"\.count\s*\(").unwrap(),
            regex::Regex::new(r"\.aggregate\s*\(").unwrap(),
            regex::Regex::new(r"\.groupBy\s*\(").unwrap(),
            regex::Regex::new(r"\.select\s*\(").unwrap(),
            regex::Regex::new(r"\.eq\s*\(").unwrap(),
            regex::Regex::new(r"SELECT\s+.+\s+FROM").unwrap(),
        ]
    });

pub(in crate::core::code_scan) static MULTI_TENANT_CONTEXT_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(
            r#"(?is)(?:session|ctx\.user|user|member|currentUser)[A-Za-z0-9_.?]*\.(?:orgId|organizationId|teamId|workspaceId|tenantId|accountId|customerId|ownerId|userId)"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)\bauth\s*\(\s*\)[A-Za-z0-9_.?]*\.(?:orgId|organizationId|teamId|workspaceId|tenantId|accountId|customerId|ownerId|userId)"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)\b(?:organizationId|orgId|teamId|workspaceId|tenantId|accountId|customerId|ownerId)\b"#,
        )
        .unwrap(),
    ]
    });

pub(in crate::core::code_scan) static TENANT_SCOPE_QUERY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(
            r#"(?is)\bwhere\s*:\s*\{[^}]{0,260}\b(?:orgId|organizationId|teamId|workspaceId|tenantId|accountId|customerId|ownerId|userId)\b"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)\bdata\s*:\s*\{[^}]{0,260}\b(?:orgId|organizationId|teamId|workspaceId|tenantId|accountId|customerId|ownerId|userId)\b"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)\.(?:eq|match|filter)\s*\(\s*["'`](?:org_id|organization_id|team_id|workspace_id|tenant_id|account_id|customer_id|owner_id|user_id|orgId|organizationId|teamId|workspaceId|tenantId|accountId|customerId|ownerId|userId)["'`]"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)\bwhere\b[^;\n]{0,220}\b(?:org_id|organization_id|team_id|workspace_id|tenant_id|account_id|customer_id|owner_id|user_id)\b"#,
        )
        .unwrap(),
    ]
    });

pub(in crate::core::code_scan) static AUTH_OWNED_ID_SCOPE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
        regex::Regex::new(
            r#"(?is)\bwhere\s*:\s*\{[^}]{0,260}\bid\s*:\s*(?:session|ctx\.user|currentUser|user|member|auth\(\))[A-Za-z0-9_.?]*(?:id|userId)"#,
        )
        .unwrap(),
        regex::Regex::new(
            r#"(?is)\.(?:eq|match|filter)\s*\(\s*["'`]id["'`]\s*,\s*(?:session|ctx\.user|currentUser|user|member|auth\(\))[A-Za-z0-9_.?]*(?:id|userId)"#,
        )
        .unwrap(),
    ]
    });

pub(in crate::core::code_scan) static TRANSACTION_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\$transaction\s*\(").unwrap(),
            regex::Regex::new(r"\btransaction\s*\(").unwrap(),
            regex::Regex::new(r"\bbeginTransaction\b").unwrap(),
            regex::Regex::new(r"\bBEGIN\b").unwrap(),
            regex::Regex::new(r"\bCOMMIT\b").unwrap(),
        ]
    });

// Template-literal queries must contain a SQL keyword so GraphQL clients do
// not become raw-SQL findings.
const SQL_KEYWORD_INSIDE_TEMPLATE: &str =
    r"(?i)(?:SELECT|INSERT|UPDATE|DELETE|WHERE|FROM|JOIN|UNION|DROP|CREATE\s+TABLE|TRUNCATE)";

pub(in crate::core::code_scan) static RAW_SQL_UNSAFE_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\$queryRawUnsafe\s*\(").unwrap(),
            regex::Regex::new(r"\$executeRawUnsafe\s*\(").unwrap(),
            // Require a SQL keyword so unrelated tagged templates do not match.
            regex::Regex::new(&format!(
                r"(?s)(?:sequelize|pool|client|db|connection)\.(?:query|execute|run)\s*\(\s*`[^`]*{}[^`]*\$\{{",
                SQL_KEYWORD_INSIDE_TEMPLATE
            ))
            .expect("static raw-sql template regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?s)(?:sequelize|pool|client|db|connection)\.(?:query|execute|run)\s*\(\s*(?:"[^"]*"\s*\+|'[^']*'\s*\+)"#).unwrap(),
            regex::Regex::new(r"(?:sqlx::query|sqlx::query_as|client\.query|conn\.query|db\.query)\s*\(\s*&?format!\s*\(").unwrap(),
            regex::Regex::new(r#"(?s)\.(?:execute|executemany|query)\s*\(\s*f["'][^"'\n]*\{"#).unwrap(),
            regex::Regex::new(r#"(?s)\.(?:execute|executemany|query)\s*\(\s*(?:"[^"]*"\s*\+|'[^']*'\s*\+)"#).unwrap(),
            // Exclude safe `$wpdb->prefix`, prepared statements, and literal PHP strings.
            regex::Regex::new(r#"(?s)->(?:query|exec|get_results|get_row|get_var|get_col|multi_query)\s*\(\s*"[^"]{0,300}\$_(?:GET|POST|REQUEST|COOKIE)"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?s)->(?:query|exec|get_results|get_row|get_var|get_col|multi_query)\s*\(\s*(?:"[^"]{0,300}"|'[^']{0,300}')\s*\.\s*\$_(?:GET|POST|REQUEST|COOKIE)"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?s)\bmysqli_(?:query|multi_query)\s*\([^,;]{0,80},\s*"[^"]{0,300}\$_(?:GET|POST|REQUEST|COOKIE)"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?s)\bmysqli_(?:query|multi_query)\s*\([^,;]{0,80},\s*(?:"[^"]{0,300}"|'[^']{0,300}')\s*\.\s*\$_(?:GET|POST|REQUEST|COOKIE)"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            // Laravel raw helpers have no benign `$wpdb->prefix` equivalent.
            regex::Regex::new(r#"(?s)\bDB::(?:select|statement|unprepared|raw)\s*\(\s*"[^"]{0,300}[{$]"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?s)\bDB::(?:select|statement|unprepared|raw)\s*\(\s*(?:"[^"]{0,300}"|'[^']{0,300}')\s*\.\s*\$"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?s)->whereRaw\s*\(\s*"[^"]{0,300}[{$]"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r#"(?s)->whereRaw\s*\(\s*(?:"[^"]{0,300}"|'[^']{0,300}')\s*\.\s*\$"#).expect("static PHP pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

#[cfg(test)]
mod db_pattern_tests {
    use super::DB_PATTERNS;

    fn touches_db(source: &str) -> bool {
        DB_PATTERNS.iter().any(|pattern| pattern.is_match(source))
    }

    #[test]
    fn matches_real_db_usage_not_prose() {
        // Genuine database usage.
        assert!(touches_db(
            "import { PrismaClient } from \"@prisma/client\";"
        ));
        assert!(touches_db("const users = await prisma.user.findMany();"));
        assert!(touches_db("import { db } from \"@/lib/prisma\";"));
        assert!(touches_db("SELECT id FROM users WHERE active"));
        assert!(touches_db("import postgres from \"postgres\";"));
        // ORM usage shapes.
        assert!(touches_db(
            "import { drizzle } from \"drizzle-orm/node-postgres\";"
        ));
        assert!(touches_db("const db = drizzle(pool);"));
        assert!(touches_db("import knex from \"knex\";"));
        assert!(touches_db("const orm = new Sequelize(url);"));
        assert!(touches_db("import mysql from \"mysql2/promise\";"));

        // Prose / instructions / config text must NOT register as DB access -
        // this is what produced a false-critical client-db-access finding.
        assert!(!touches_db(
            "<li>Run: npx prisma db push (to sync schema)</li>"
        ));
        assert!(!touches_db("// build the SQL query string here"));
        assert!(!touches_db(
            "Set DATABASE_URL to a postgres connection string"
        ));

        assert!(!touches_db("import { Post } from \"@prisma/client\";"));
        assert!(!touches_db("import type { User } from \"@prisma/client\";"));

        assert!(!touches_db(
            "// drizzle pulls in node-only deps via the schema"
        ));
        assert!(!touches_db(
            "// duplicated from the knex migration for clarity"
        ));
        assert!(!touches_db(
            "a comment mentioning sequelize and mysql in passing"
        ));
    }
}
