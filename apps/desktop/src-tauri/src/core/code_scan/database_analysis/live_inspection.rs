use super::*;

mod postgres;
mod sqlite;

pub(in crate::core::code_scan) use postgres::collect_local_postgres_snapshots;
pub(in crate::core::code_scan) use sqlite::collect_local_sqlite_snapshots;

pub(in crate::core::code_scan) fn infer_relation_targets_from_column(column: &str) -> Vec<String> {
    let lower = column.to_ascii_lowercase();
    let base = if let Some(stripped) = lower.strip_suffix("_id") {
        stripped
    } else if let Some(stripped) = lower.strip_suffix("id") {
        stripped
    } else {
        return Vec::new();
    }
    .trim_end_matches('_');

    if base.is_empty() {
        return Vec::new();
    }

    let mut targets = HashSet::new();
    targets.insert(base.to_string());
    if !base.ends_with('s') {
        targets.insert(format!("{}s", base));
    }
    if let Some(stem) = base.strip_suffix('y') {
        targets.insert(format!("{}ies", stem));
    }

    let mut collected = targets.into_iter().collect::<Vec<_>>();
    collected.sort();
    collected
}

pub(in crate::core::code_scan) fn is_join_like_metadata_column(column: &str) -> bool {
    matches!(
        column.to_ascii_lowercase().as_str(),
        "id" | "role"
            | "status"
            | "type"
            | "kind"
            | "scope"
            | "position"
            | "sort_order"
            | "sortorder"
            | "ordinal"
            | "priority"
            | "is_primary"
            | "isprimary"
            | "created_at"
            | "createdat"
            | "updated_at"
            | "updatedat"
            | "deleted_at"
            | "deletedat"
            | "expires_at"
            | "expiresat"
            | "starts_at"
            | "startsat"
            | "ends_at"
            | "endsat"
    )
}
