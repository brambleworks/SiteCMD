//! Project and environment CRUD, tier checks, score trend.

use super::DbError;
use rusqlite::{named_params, params, OptionalExtension};

use super::from_row;
use super::helpers::{normalize_env_url, normalize_url};
use super::types::{EnvironmentRecord, ProjectRecord, ScoreTrendPoint};
use super::Database;

fn generate_secret_namespace() -> Result<String, DbError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|e| DbError::Other(format!("Failed to generate secret namespace: {}", e)))?;
    Ok(hex::encode(bytes))
}

fn environment_preference_score(environment: &EnvironmentRecord) -> u8 {
    let environment_score = if environment.environment == "local" {
        0
    } else {
        4
    };
    let source_score = if environment.source.as_deref() == Some("manual") {
        0
    } else {
        2
    };
    let url_score = if environment.url.contains("localhost") {
        0
    } else {
        1
    };
    environment_score + source_score + url_score
}

fn dedupe_environment_records(environments: Vec<EnvironmentRecord>) -> Vec<EnvironmentRecord> {
    let mut deduped: Vec<EnvironmentRecord> = Vec::new();

    for environment in environments {
        // Shared with project detection so env dedupe and detection agree on
        // URL identity.
        let key = crate::core::project::url_identity_key(&environment.url);
        if let Some(existing_index) = deduped
            .iter()
            .position(|existing| crate::core::project::url_identity_key(&existing.url) == key)
        {
            if environment_preference_score(&environment)
                < environment_preference_score(&deduped[existing_index])
            {
                deduped[existing_index] = environment;
            }
        } else {
            deduped.push(environment);
        }
    }

    deduped
}

impl Database {
    /// Add or update a project, returns project_id
    #[tracing::instrument(skip(self, path), fields(name = %name, has_project_path = !path.trim().is_empty(), framework = ?framework))]
    pub fn upsert_project(
        &self,
        name: &str,
        path: &str,
        framework: Option<&str>,
    ) -> Result<i64, DbError> {
        let name = name.to_string();
        let path = path.to_string();
        let framework = framework.map(|s| s.to_string());
        self.execute(move |conn| {
            let existing: Option<i64> = if !path.is_empty() {
                conn.query_row(
                    "SELECT id FROM projects WHERE path = :path",
                    named_params! { ":path": path },
                    |r| r.get(0),
                )
                .ok()
            } else {
                None
            };

            if let Some(id) = existing {
                let existing_namespace: Option<String> = conn
                    .query_row(
                        "SELECT secret_namespace FROM projects WHERE id = :id",
                        named_params! { ":id": id },
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .optional()?
                    .flatten();
                let secret_namespace = existing_namespace
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(generate_secret_namespace()?);
                conn.execute(
                    "UPDATE projects
                     SET name = :name,
                         framework = :framework,
                         secret_namespace = :secret_namespace
                     WHERE id = :id",
                    named_params! {
                        ":name": name,
                        ":framework": framework,
                        ":secret_namespace": secret_namespace,
                        ":id": id
                    },
                )?;
                return Ok(id);
            }

            let secret_namespace = generate_secret_namespace()?;
            conn.execute(
                "INSERT INTO projects (name, path, framework, secret_namespace)
                 VALUES (:name, :path, :framework, :secret_namespace)",
                named_params! {
                    ":name": name,
                    ":path": path,
                    ":framework": framework,
                    ":secret_namespace": secret_namespace
                },
            )?;
            Ok(conn.last_insert_rowid())
        })?
    }

    /// Rename a project by ID.
    #[tracing::instrument(skip(self), fields(project_id, name = %name))]
    pub fn rename_project(&self, project_id: i64, name: &str) -> Result<(), DbError> {
        let name = name.to_string();
        self.execute(move |conn| {
            conn.execute(
                "UPDATE projects SET name = ?1 WHERE id = ?2",
                params![name, project_id],
            )?;
            Ok(())
        })?
    }

    /// Update a project's local path and framework.
    #[tracing::instrument(skip(self, path), fields(project_id, has_project_path = !path.trim().is_empty(), framework = ?framework))]
    pub fn update_project_path(
        &self,
        project_id: i64,
        path: &str,
        framework: Option<&str>,
    ) -> Result<(), DbError> {
        let path = path.to_string();
        let framework = framework.map(|s| s.to_string());
        self.execute(move |conn| {
            conn.execute(
                "UPDATE projects SET path = ?1, framework = ?2 WHERE id = ?3",
                params![path, framework, project_id],
            )?;
            Ok(())
        })?
    }

    /// Strict project lookup for scan persistence and other accuracy-sensitive
    /// workflows. A genuinely unlinked URL is `Ok(None)`; storage failures stay
    /// errors so callers cannot skip canonical issue persistence.
    #[tracing::instrument(skip(self, url))]
    pub fn find_project_for_url_result(&self, url: &str) -> Result<Option<i64>, DbError> {
        let url = url.to_string();
        Ok(self.execute(move |conn| {
            let (normalized, url_slash) = normalize_url(&url);
            // ORDER BY id ASC keeps the pick deterministic when two projects
            // registered the same URL: the oldest environment row wins instead
            // of whichever row SQLite happens to visit first.
            conn.query_row(
                "SELECT project_id FROM environments WHERE url = ?1 OR url = ?2
                 ORDER BY id ASC LIMIT 1",
                params![normalized, url_slash],
                |row| row.get(0),
            )
            .optional()
        })??)
    }

    /// Best-effort project lookup for routing and notification copy.
    #[tracing::instrument(skip(self, url))]
    pub fn find_project_for_url(&self, url: &str) -> Option<i64> {
        self.find_project_for_url_result(url).ok().flatten()
    }

    /// Best-effort project-name lookup for user-facing copy.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_project_name(&self, project_id: i64) -> Option<String> {
        self.execute(move |conn| {
            conn.query_row(
                "SELECT name FROM projects WHERE id = ?1",
                params![project_id],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .filter(|name| !name.is_empty())
        })
        .ok()
        .flatten()
    }

    /// Strict local-path read for issue construction and other accuracy-sensitive
    /// workflows. A missing project/path is `Ok(None)`; storage failures remain
    /// errors so callers cannot misreport "no linked folder."
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_project_path_result(&self, project_id: i64) -> Result<Option<String>, DbError> {
        Ok(self.execute(move |conn| {
            conn.query_row(
                "SELECT path FROM projects WHERE id = ?1",
                params![project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|path| path.filter(|value| !value.is_empty()))
        })??)
    }

    /// Best-effort project path lookup; returns `None` when unavailable.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_project_path(&self, project_id: i64) -> Option<String> {
        self.get_project_path_result(project_id).ok().flatten()
    }

    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn ensure_project_secret_namespace(&self, project_id: i64) -> Result<String, DbError> {
        self.execute(move |conn| {
            let existing: Option<String> = conn
                .query_row(
                    "SELECT secret_namespace FROM projects WHERE id = ?1",
                    params![project_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten();

            if let Some(namespace) = existing.filter(|value| !value.trim().is_empty()) {
                return Ok(namespace);
            }

            let namespace = generate_secret_namespace()?;
            let updated = conn.execute(
                "UPDATE projects SET secret_namespace = ?1 WHERE id = ?2",
                params![namespace, project_id],
            )?;
            if updated == 0 {
                return Err(format!("Project {} not found", project_id).into());
            }
            Ok(namespace)
        })?
    }

    /// Resolve the effective connected-service tier, including offline grace.
    /// Returns Free if license state cannot be read.
    #[tracing::instrument(skip(self))]
    pub fn get_effective_tier(&self) -> crate::licensing::config::Tier {
        self.execute(|conn| {
            crate::licensing::store::load(conn)
                .ok()
                .flatten()
                .as_ref()
                .map(crate::licensing::access::effective_tier_from_state)
                .unwrap_or(crate::licensing::config::Tier::Free)
        })
        .unwrap_or(crate::licensing::config::Tier::Free)
    }

    /// Best-effort latest critical/high counts; returns zeroes when unavailable.
    #[tracing::instrument(skip(self, url))]
    pub fn get_latest_issue_counts(&self, url: &str) -> (u32, u32) {
        let url = url.to_string();
        self.execute(move |conn| {
            let normalized = normalize_url(&url).0;
            conn.query_row(
                "SELECT critical_count, high_count
                 FROM score_snapshots
                 WHERE environment_url = ?1
                 ORDER BY computed_at DESC, id DESC LIMIT 1",
                params![normalized],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?)),
            )
            .unwrap_or((0, 0))
        })
        .unwrap_or((0, 0))
    }

    /// Batch critical/high counts, omitting URLs without scans.
    #[tracing::instrument(skip(self, urls))]
    pub fn get_latest_issue_counts_batch(
        &self,
        urls: &[String],
    ) -> Result<std::collections::HashMap<String, (u32, u32)>, DbError> {
        if urls.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let urls_owned: Vec<String> = urls.to_vec();
        self.execute(move |conn| {
            let mut result = std::collections::HashMap::new();
            for url in &urls_owned {
                let normalized = normalize_url(url).0;
                let counts = conn
                    .query_row(
                        "SELECT critical_count, high_count
                         FROM score_snapshots
                         WHERE environment_url = ?1
                         ORDER BY computed_at DESC, id DESC LIMIT 1",
                        rusqlite::params![normalized],
                        |row| Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?)),
                    )
                    .optional()?;
                if let Some((c, h)) = counts {
                    result.insert(url.clone(), (c, h));
                }
            }
            Ok::<_, DbError>(result)
        })?
    }

    /// Count total projects.
    #[tracing::instrument(skip(self))]
    pub fn get_project_count(&self) -> Result<u32, DbError> {
        self.execute(|conn| {
            conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
                .map_err(DbError::from)
        })?
    }

    /// Add a normalized environment URL to a project.
    #[tracing::instrument(skip(self, url), fields(project_id, label = %label, environment = %environment, source = %source))]
    pub fn add_environment(
        &self,
        project_id: i64,
        url: &str,
        label: &str,
        environment: &str,
        source: &str,
    ) -> Result<i64, DbError> {
        let url = normalize_env_url(Some(url));
        let label = label.to_string();
        let environment = environment.to_string();
        let source = source.to_string();
        self.execute(move |conn| {
            let existing: Option<i64> = conn
                .query_row("SELECT id FROM environments WHERE project_id = :project_id AND url = :url",
                    named_params! { ":project_id": project_id, ":url": url }, |r| r.get(0))
                .optional()?;

            if let Some(id) = existing {
                conn.execute(
                    "UPDATE environments SET label = :label, environment = :environment, source = :source WHERE id = :id",
                    named_params! { ":label": label, ":environment": environment, ":source": source, ":id": id },
                )?;
                return Ok(id);
            }

            conn.execute(
                "INSERT INTO environments (project_id, url, label, environment, source) VALUES (:project_id, :url, :label, :environment, :source)",
                named_params! { ":project_id": project_id, ":url": url, ":label": label, ":environment": environment, ":source": source },
            )?;
            let environment_id = conn.last_insert_rowid();

            // Attach matching ad-hoc scan history without replacing an existing site.
            conn.execute(
                "UPDATE OR IGNORE sites SET project_id = :project_id
                 WHERE project_id IS NULL AND (url = :url OR url = :url || '/')",
                named_params! { ":project_id": project_id, ":url": url },
            )
            ?;

            Ok(environment_id)
        })?
    }

    /// Whether `env_url` is this project's production environment.
    #[tracing::instrument(skip(self, env_url), fields(project_id))]
    pub fn environment_is_production(
        &self,
        project_id: i64,
        env_url: &str,
    ) -> Result<bool, DbError> {
        // Environments are stored normalized (see add_environment).
        let env_url = normalize_env_url(Some(env_url));
        self.execute(move |conn| {
            conn.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM environments
                    WHERE project_id = :project_id AND url = :url
                      AND environment = 'production'
                 )",
                named_params! { ":project_id": project_id, ":url": env_url },
                |row| row.get::<_, bool>(0),
            )
            .map_err(DbError::from)
        })?
    }

    /// Get all projects with their environments
    #[tracing::instrument(skip(self))]
    pub fn get_projects(&self) -> Result<Vec<ProjectRecord>, DbError> {
        self.execute(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, name, path, framework, created_at, secret_namespace
                     FROM projects
                     ORDER BY created_at DESC",
                )
                ?;
            let projects: Vec<ProjectRecord> = from_row::query_vec(&mut stmt, &[])?;

            let mut result = Vec::new();
            for mut project in projects {
                let mut env_stmt = conn
                    .prepare(
                        "SELECT e.id AS id, e.url AS url, e.label AS label, e.environment AS environment,
                                e.source AS source, e.last_scanned_at AS last_scanned_at,
                                (SELECT CAST(ROUND(snapshot.overall) AS INTEGER)
                                 FROM score_snapshots snapshot
                                 WHERE snapshot.project_id = e.project_id
                                   AND snapshot.environment_url = e.url
                                 ORDER BY snapshot.computed_at DESC, snapshot.id DESC
                                 LIMIT 1) AS latest_score
                         FROM environments e WHERE e.project_id = ?1
                         ORDER BY
                            CASE e.environment
                                WHEN 'production' THEN 0
                                WHEN 'staging' THEN 1
                                WHEN 'development' THEN 2
                                WHEN 'local' THEN 3
                                ELSE 4
                            END"
                    )
                    ?;
                project.environments =
                    dedupe_environment_records(from_row::query_vec(&mut env_stmt, &[&project.id])?);

                result.push(project);
            }
            Ok(result)
        })?
    }

    /// Delete project-owned executions and cascade remaining project data.
    /// Ad-hoc sites with no project remain untouched.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn delete_project(&self, project_id: i64) -> Result<(), DbError> {
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM scan_executions WHERE project_id = ?1",
                params![project_id],
            )?;
            tx.execute("DELETE FROM projects WHERE id = ?1", params![project_id])?;
            tx.commit()?;
            Ok(())
        })?
    }

    /// Delete an environment, its owned scans, and URL-keyed state that has no
    /// foreign key to the environment row.
    #[tracing::instrument(skip(self), fields(environment_id))]
    pub fn delete_environment(&self, environment_id: i64) -> Result<(), DbError> {
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            let row: Option<(i64, String)> = tx
                .query_row(
                    "SELECT project_id, url FROM environments WHERE id = ?1",
                    params![environment_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((project_id, url)) = row {
                let (normalized, url_slash) = normalize_url(&url);
                tx.execute(
                    "DELETE FROM scan_executions
                     WHERE environment_id = ?1
                        OR (project_id = ?2 AND (
                            environment_scope_key = ?3
                            OR environment_scope_key = ?4
                            OR environment_url = ?3
                            OR environment_url = ?4
                        ))",
                    params![environment_id, project_id, normalized, url_slash],
                )?;
                tx.execute(
                    "DELETE FROM sites WHERE project_id = ?1 AND (url = ?2 OR url = ?3)",
                    params![project_id, normalized, url_slash],
                )?;
                for (table, env_column) in [
                    ("work_items", "env_url"),
                    ("project_issue_states", "env_url"),
                    ("alerts", "env_url"),
                    ("fix_attempts", "env_url"),
                    ("regressions", "env_url"),
                    ("project_signal_snapshots", "environment_url"),
                    // score_snapshots stores only the normalized env URL, so
                    // the normalized/slash variants below already match it.
                    ("score_snapshots", "environment_url"),
                ] {
                    tx.execute(
                        &format!(
                            "DELETE FROM {table} WHERE project_id = ?1 \
                             AND ({column} = ?2 OR {column} = ?3 OR {column} = ?4)",
                            table = table,
                            column = env_column
                        ),
                        params![project_id, normalized, url_slash, url],
                    )?;
                }
            }
            tx.execute(
                "DELETE FROM environments WHERE id = ?1",
                params![environment_id],
            )?;
            tx.commit()?;
            Ok(())
        })?
    }

    /// Lists project environments with production first per project.
    #[tracing::instrument(skip(self))]
    pub fn list_all_project_envs(&self) -> Result<Vec<(i64, String)>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT project_id, url FROM environments
                 ORDER BY project_id,
                          CASE WHEN environment = 'production' THEN 0 ELSE 1 END,
                          id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    /// Returns all environment URLs for a given project, production first.
    /// Used by the integration scheduler for immediate per-project polls.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn list_project_envs(&self, project_id: i64) -> Result<Vec<String>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT url FROM environments WHERE project_id = ?1
                 ORDER BY CASE WHEN environment = 'production' THEN 0 ELSE 1 END, id",
            )?;
            let rows =
                stmt.query_map(rusqlite::params![project_id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    /// Get score trend data for a URL (for chart)
    #[tracing::instrument(skip(self, url), fields(limit))]
    pub fn get_score_trend(&self, url: &str, limit: u32) -> Result<Vec<ScoreTrendPoint>, DbError> {
        self.get_score_trend_for_scope(None, url, limit)
    }

    /// Get score trend data for one explicitly selected project environment.
    #[tracing::instrument(skip(self, url), fields(project_id, limit))]
    pub fn get_score_trend_for_project(
        &self,
        project_id: i64,
        url: &str,
        limit: u32,
    ) -> Result<Vec<ScoreTrendPoint>, DbError> {
        self.get_score_trend_for_scope(Some(project_id), url, limit)
    }

    fn get_score_trend_for_scope(
        &self,
        project_id: Option<i64>,
        url: &str,
        limit: u32,
    ) -> Result<Vec<ScoreTrendPoint>, DbError> {
        let url = url.to_string();
        self.execute(move |conn| {
            let normalized = normalize_url(&url).0;
            let mut stmt = conn.prepare(
                    "SELECT recent.overall_score, recent.security_score, recent.performance_score,
                            recent.seo_score, recent.accessibility_score, recent.compliance_score,
                            recent.config_score, recent.polish_score, recent.timestamp, recent.issues_total,
                            recent.scan_type
                     FROM (
                       SELECT run.raw_score AS overall_score,
                              run.security_score, run.performance_score,
                              run.seo_score, run.accessibility_score,
                              run.compliance_score, run.config_score,
                              run.polish_score, run.timestamp_text AS timestamp,
                              run.issues_total,
                              COALESCE(run.focus, 'health') AS scan_type
                       FROM scan_runs run
                       WHERE run.source = 'web_scan'
                         AND run.run_kind IN ('single', 'page')
                         AND run.status = 'complete'
                         AND run.environment_scope_key = :environment_scope_key
                         AND (:project_id IS NULL OR run.project_id = :project_id)
                       ORDER BY run.started_at DESC, run.id DESC
                       LIMIT :limit
                     ) recent
                     ORDER BY recent.timestamp ASC",
                )?;
            let rows = stmt.query_map(
                named_params! {
                    ":environment_scope_key": normalized,
                    ":project_id": project_id,
                    ":limit": limit,
                },
                <ScoreTrendPoint as from_row::FromRow>::from_row,
            )?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?
    }
}
