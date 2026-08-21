use super::*;

mod drizzle_integrity;
mod expected_shape;
mod migrations;
mod prisma_integrity;

pub(in crate::core::code_scan) use expected_shape::{
    collect_expected_db_columns, collect_expected_db_table_names,
};
pub(in crate::core::code_scan) use migrations::{
    collect_expected_drizzle_migration_names, collect_expected_prisma_migration_names,
};

use drizzle_integrity::collect_drizzle_schema_integrity_issues;
use prisma_integrity::collect_prisma_schema_integrity_issues;

pub(in crate::core::code_scan) fn collect_source_schema_integrity_issues(
    artifacts: &[TextArtifact],
) -> Vec<CodeIssue> {
    let mut issues = Vec::new();

    for artifact in artifacts {
        issues.extend(collect_prisma_schema_integrity_issues(artifact));
        issues.extend(collect_drizzle_schema_integrity_issues(artifact));
    }

    issues
}
