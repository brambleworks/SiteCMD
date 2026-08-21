//! Integration config CRUD.

use super::DbError;
use rusqlite::{named_params, params};

use super::helpers::parse_required_enum;
use super::Database;

impl Database {
    /// Save or update an integration config (INSERT OR REPLACE by project_id + type).
    #[tracing::instrument(skip(self, config), fields(project_id))]
    pub fn save_integration(
        &self,
        project_id: i64,
        config: &crate::integrations::IntegrationConfig,
    ) -> Result<(), DbError> {
        let config = config.clone();
        self.execute(move |conn| {
            let type_str = serde_json::to_string(&config.integration_type)?
                .trim_matches('"')
                .to_string();
            let extra_str = config
                .extra
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;

            conn.execute(
                "INSERT OR REPLACE INTO integration_configs (project_id, integration_type, api_key, site_id, extra, enabled)
                 VALUES (:project_id, :integration_type, :api_key, :site_id, :extra, :enabled)",
                named_params! {
                    ":project_id": project_id,
                    ":integration_type": type_str,
                    ":api_key": config.api_key,
                    ":site_id": config.site_id,
                    ":extra": extra_str,
                    ":enabled": config.enabled as i32,
                },
            )?;
            Ok(())
        })?
    }

    /// Get all integration configs for a project. API keys may contain keyring placeholders.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_integrations(
        &self,
        project_id: i64,
    ) -> Result<Vec<crate::integrations::IntegrationConfig>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT integration_type, api_key, site_id, extra, enabled FROM integration_configs WHERE project_id = ?1"
            )?;

            let rows = stmt.query_map(params![project_id], |row| {
                let type_str: String = row.get(0)?;
                let extra_str: Option<String> = row.get(3)?;
                let extra = extra_str
                    .as_deref()
                    .map(|json| {
                        serde_json::from_str(json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })
                    })
                    .transpose()?;
                Ok(crate::integrations::IntegrationConfig {
                    integration_type: parse_required_enum(
                        0,
                        "integration_configs.integration_type",
                        &type_str,
                    )?,
                    api_key: row.get(1)?,
                    site_id: row.get(2)?,
                    extra,
                    enabled: row.get::<_, i32>(4)? != 0,
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    /// Delete an integration config by project_id and type string.
    #[tracing::instrument(skip(self), fields(project_id, integration_type = %integration_type))]
    pub fn delete_integration(
        &self,
        project_id: i64,
        integration_type: &str,
    ) -> Result<(), DbError> {
        let integration_type = integration_type.to_string();
        self.execute(move |conn| {
            conn.execute(
                "DELETE FROM integration_configs WHERE project_id = ?1 AND integration_type = ?2",
                params![project_id, integration_type],
            )?;
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self), fields(project_id, check_id = %check_id, integration_type = %integration_type))]
    pub fn dismiss_integration_hint(
        &self,
        project_id: i64,
        check_id: &str,
        integration_type: &str,
    ) -> Result<(), DbError> {
        let check_id = check_id.to_string();
        let integration_type = integration_type.to_string();
        self.execute(move |conn| {
            let now_ms = chrono::Utc::now().timestamp_millis();
            conn.execute(
                "INSERT OR REPLACE INTO dismissed_integration_hints
                 (project_id, check_id, integration_type, dismissed_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![project_id, check_id, integration_type, now_ms],
            )?;
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_dismissed_integration_hints(
        &self,
        project_id: i64,
    ) -> Result<Vec<(String, String)>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT check_id, integration_type
                 FROM dismissed_integration_hints
                 WHERE project_id = ?1",
            )?;
            let rows = stmt.query_map(params![project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_connected_integration_types(&self, project_id: i64) -> Result<Vec<String>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT integration_type FROM integration_configs
                 WHERE project_id = ?1 AND enabled = 1",
            )?;
            let rows = stmt.query_map(params![project_id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    /// Update only the api_key column for a specific integration (used during keychain migration)
    #[tracing::instrument(skip(self, new_key), fields(project_id, integration_type = %integration_type))]
    pub fn update_integration_api_key(
        &self,
        project_id: i64,
        integration_type: &str,
        new_key: &str,
    ) -> Result<(), DbError> {
        let integration_type = integration_type.to_string();
        let new_key = new_key.to_string();
        self.execute(move |conn| {
            conn.execute(
                "UPDATE integration_configs SET api_key = :api_key WHERE project_id = :project_id AND integration_type = :integration_type",
                named_params! { ":project_id": project_id, ":integration_type": integration_type, ":api_key": new_key },
            )?;
            Ok(())
        })?
    }
}
