use super::*;

#[derive(Debug, Clone)]
pub(in crate::core::code_scan) struct TextArtifact {
    pub(in crate::core::code_scan) absolute_path: PathBuf,
    pub(in crate::core::code_scan) relative_path: String,
    pub(in crate::core::code_scan) content: String,
}

#[derive(Debug, Clone)]
pub(in crate::core::code_scan) struct SupabaseTableAccess {
    pub(in crate::core::code_scan) relative_path: String,
    pub(in crate::core::code_scan) table: String,
    pub(in crate::core::code_scan) line: Option<u32>,
    pub(in crate::core::code_scan) operations: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(in crate::core::code_scan) struct LocalRlsPolicyState {
    pub(in crate::core::code_scan) relative_path: String,
    pub(in crate::core::code_scan) absolute_path: String,
    pub(in crate::core::code_scan) line: Option<u32>,
    pub(in crate::core::code_scan) auth_scoped: bool,
    pub(in crate::core::code_scan) permissive: bool,
    pub(in crate::core::code_scan) operations: HashSet<String>,
    pub(in crate::core::code_scan) roles: Vec<String>,
    pub(in crate::core::code_scan) applies_to_frontend_roles: bool,
}

#[derive(Debug, Clone, Default)]
pub(in crate::core::code_scan) struct LocalRlsTableState {
    pub(in crate::core::code_scan) enabled: bool,
    pub(in crate::core::code_scan) policies: Vec<LocalRlsPolicyState>,
}

#[derive(Debug, Clone)]
pub(in crate::core::code_scan) struct LocalSqliteSnapshot {
    pub(in crate::core::code_scan) absolute_path: PathBuf,
    pub(in crate::core::code_scan) relative_path: String,
    pub(in crate::core::code_scan) tables: Vec<LocalDbTableSnapshot>,
    pub(in crate::core::code_scan) has_prisma_migrations_table: bool,
    pub(in crate::core::code_scan) applied_prisma_migrations: HashSet<String>,
    pub(in crate::core::code_scan) has_drizzle_migrations_table: bool,
    pub(in crate::core::code_scan) applied_drizzle_migration_count: usize,
}

#[derive(Debug, Clone)]
pub(in crate::core::code_scan) struct LocalDbTableSnapshot {
    pub(in crate::core::code_scan) name: String,
    pub(in crate::core::code_scan) columns: Vec<String>,
    pub(in crate::core::code_scan) non_null_columns: HashSet<String>,
    pub(in crate::core::code_scan) indexed_columns: HashSet<String>,
    pub(in crate::core::code_scan) unique_indexed_columns: HashSet<String>,
    pub(in crate::core::code_scan) unique_index_groups: Vec<HashSet<String>>,
    pub(in crate::core::code_scan) foreign_key_columns: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(in crate::core::code_scan) struct LocalPostgresSnapshot {
    pub(in crate::core::code_scan) absolute_path: PathBuf,
    pub(in crate::core::code_scan) relative_path: String,
    pub(in crate::core::code_scan) database_name: Option<String>,
    pub(in crate::core::code_scan) host: Option<String>,
    pub(in crate::core::code_scan) tables: Vec<LocalDbTableSnapshot>,
    pub(in crate::core::code_scan) has_prisma_migrations_table: bool,
    pub(in crate::core::code_scan) applied_prisma_migrations: HashSet<String>,
    pub(in crate::core::code_scan) has_drizzle_migrations_table: bool,
    pub(in crate::core::code_scan) applied_drizzle_migration_count: usize,
}

#[derive(Debug, Clone)]
pub(in crate::core::code_scan) struct EnvFileSnapshot {
    pub(in crate::core::code_scan) absolute_path: PathBuf,
    pub(in crate::core::code_scan) relative_path: String,
    pub(in crate::core::code_scan) content: String,
    pub(in crate::core::code_scan) keys: HashSet<String>,
    pub(in crate::core::code_scan) entries: HashMap<String, String>,
}
