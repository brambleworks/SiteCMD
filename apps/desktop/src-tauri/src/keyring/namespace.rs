pub(super) fn project_secret_namespace(
    db: &crate::db::Database,
    project_id: i64,
) -> Result<String, String> {
    db.ensure_project_secret_namespace(project_id)
        .map_err(String::from)
}
