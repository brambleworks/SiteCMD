//! Idempotent deploy-regression records keyed by scan type and canonical run.
//! Stored check IDs represent introduced findings only.

use super::DbError;
use rusqlite::{named_params, OptionalExtension};

use super::helpers::normalize_env_url;
use super::Database;

#[derive(Debug, Clone)]
pub struct RegressionInput {
    pub project_id: i64,
    pub env_url: String,
    pub scan_type: String, // "web" | "code"
    /// Canonical run id retained under the API's historical field name.
    pub prev_scan_id: i64,
    /// Canonical run id retained under the API's historical field name.
    pub scan_id: i64,
    pub prev_score: i64,
    pub score: i64,
    pub commit_from: String,
    pub commit_to: String,
    pub commit_count: i64,
    pub commits_json: String,
    pub new_check_ids: Vec<String>,
    pub fixed_check_ids_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct RegressionRow {
    pub id: i64,
    pub project_id: i64,
    pub env_url: String,
    pub scan_type: String,
    pub prev_scan_id: i64,
    pub scan_id: i64,
    pub prev_score: i64,
    pub score: i64,
    pub commit_from: String,
    pub commit_to: String,
    pub commit_count: i64,
    pub commits_json: String,
    pub fixed_check_ids_json: String,
    pub created_at: i64,
}

impl Database {
    /// Insert a regression and its introduced check_ids. Idempotent: a second
    /// insert for the same (scan_type, canonical run id) returns the existing row id
    /// without modifying anything.
    #[tracing::instrument(skip(self, input), fields(project_id = input.project_id, scan_type = %input.scan_type, scan_id = input.scan_id))]
    pub fn insert_regression(&self, input: RegressionInput) -> Result<i64, DbError> {
        let env_key = normalize_env_url(Some(&input.env_url));
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM regressions WHERE scan_type = :scan_type AND run_id = :scan_id",
                    named_params! { ":scan_type": input.scan_type, ":scan_id": input.scan_id },
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(id) = existing {
                return Ok(id);
            }

            tx.execute(
                "INSERT INTO regressions (
                    project_id, env_url, scan_type, prev_run_id, run_id,
                    prev_score, score, commit_from, commit_to, commit_count,
                    commits_json, fixed_check_ids_json, created_at
                 ) VALUES (
                    :project_id, :env_url, :scan_type, :prev_scan_id, :scan_id,
                    :prev_score, :score, :commit_from, :commit_to, :commit_count,
                    :commits_json, :fixed_check_ids_json, :created_at
                 )",
                named_params! {
                    ":project_id": input.project_id,
                    ":env_url": env_key,
                    ":scan_type": input.scan_type,
                    ":prev_scan_id": input.prev_scan_id,
                    ":scan_id": input.scan_id,
                    ":prev_score": input.prev_score,
                    ":score": input.score,
                    ":commit_from": input.commit_from,
                    ":commit_to": input.commit_to,
                    ":commit_count": input.commit_count,
                    ":commits_json": input.commits_json,
                    ":fixed_check_ids_json": input.fixed_check_ids_json,
                    ":created_at": input.created_at,
                },
            )?;
            let regression_id = tx.last_insert_rowid();

            for check_id in &input.new_check_ids {
                tx.execute(
                    "INSERT OR IGNORE INTO regression_check_ids (regression_id, check_id)
                     VALUES (:regression_id, :check_id)",
                    named_params! { ":regression_id": regression_id, ":check_id": check_id },
                )?;
            }

            tx.commit()?;
            Ok(regression_id)
        })?
    }

    /// Introduced check_ids for one regression, sorted for determinism.
    #[tracing::instrument(skip(self), fields(regression_id))]
    pub fn get_regression_check_ids(&self, regression_id: i64) -> Result<Vec<String>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT check_id FROM regression_check_ids
                     WHERE regression_id = :regression_id ORDER BY check_id",
            )?;
            let rows = stmt
                .query_map(named_params! { ":regression_id": regression_id }, |row| {
                    row.get::<_, String>(0)
                })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self), fields(scan_type = %scan_type, scan_id))]
    pub fn get_regression_by_scan(
        &self,
        scan_type: &str,
        scan_id: i64,
    ) -> Result<Option<RegressionRow>, DbError> {
        let scan_type = scan_type.to_string();
        self.execute(move |conn| {
            conn.query_row(
                "SELECT id, project_id, env_url, scan_type,
                        prev_run_id AS prev_scan_id, run_id AS scan_id,
                        prev_score, score, commit_from, commit_to, commit_count,
                        commits_json, fixed_check_ids_json, created_at
                 FROM regressions WHERE scan_type = :scan_type AND run_id = :scan_id",
                named_params! { ":scan_type": scan_type, ":scan_id": scan_id },
                |row| {
                    Ok(RegressionRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        env_url: row.get(2)?,
                        scan_type: row.get(3)?,
                        prev_scan_id: row.get(4)?,
                        scan_id: row.get(5)?,
                        prev_score: row.get(6)?,
                        score: row.get(7)?,
                        commit_from: row.get(8)?,
                        commit_to: row.get(9)?,
                        commit_count: row.get(10)?,
                        commits_json: row.get(11)?,
                        fixed_check_ids_json: row.get(12)?,
                        created_at: row.get(13)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::from)
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::temp_db;

    fn input(project_id: i64, scan_id: i64) -> RegressionInput {
        RegressionInput {
            project_id,
            env_url: "https://example.com".into(),
            scan_type: "web".into(),
            prev_scan_id: 10,
            scan_id,
            prev_score: 92,
            score: 84,
            commit_from: "aaa111".into(),
            commit_to: "bbb222".into(),
            commit_count: 3,
            commits_json: "[]".into(),
            new_check_ids: vec!["seo.meta-description".into(), "security.csp-header".into()],
            fixed_check_ids_json: "[]".into(),
            created_at: 1_770_000_000_000,
        }
    }

    fn test_project(db: &crate::db::test_helpers::TestDb) -> i64 {
        db.upsert_project("test", "/tmp/sitecmd-regressions-test", None)
            .expect("project")
    }

    #[test]
    fn insert_regression_persists_row_and_check_ids() {
        let db = temp_db();
        let project_id = test_project(&db);
        let id = db.insert_regression(input(project_id, 11)).expect("insert");
        let row = db
            .get_regression_by_scan("web", 11)
            .expect("get")
            .expect("row exists");
        assert_eq!(row.id, id);
        assert_eq!(row.score, 84);
        assert_eq!(row.commit_count, 3);
        let check_ids = db.get_regression_check_ids(id).expect("check ids");
        assert_eq!(
            check_ids,
            vec![
                "security.csp-header".to_string(),
                "seo.meta-description".to_string()
            ]
        );
    }

    #[test]
    fn insert_regression_normalizes_env_url() {
        let db = temp_db();
        let project_id = test_project(&db);
        let mut messy = input(project_id, 11);
        // normalize_env_url lowercases the host and strips the trailing slash,
        // so the stored row must hold the clean key form.
        messy.env_url = "https://Example.COM/".into();
        db.insert_regression(messy).expect("insert");
        let row = db
            .get_regression_by_scan("web", 11)
            .expect("get")
            .expect("row exists");
        assert_eq!(row.env_url, "https://example.com");
    }

    #[test]
    fn insert_regression_is_idempotent_per_scan() {
        let db = temp_db();
        let project_id = test_project(&db);
        let first = db.insert_regression(input(project_id, 11)).expect("first");
        let mut second_input = input(project_id, 11);
        second_input.score = 1; // would change the row if not idempotent
        let second = db.insert_regression(second_input).expect("second");
        assert_eq!(first, second);
        let row = db.get_regression_by_scan("web", 11).unwrap().unwrap();
        assert_eq!(row.score, 84, "second insert must not modify the row");
    }

    #[test]
    fn same_scan_id_different_scan_type_is_a_distinct_regression() {
        let db = temp_db();
        let project_id = test_project(&db);
        let web = db.insert_regression(input(project_id, 11)).expect("web");
        let mut code = input(project_id, 11);
        code.scan_type = "code".into();
        let code_id = db.insert_regression(code).expect("code");
        assert_ne!(web, code_id);
    }
}
