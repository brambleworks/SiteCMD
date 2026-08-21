//! Webhook configuration CRUD.

use super::DbError;
use rusqlite::{named_params, OptionalExtension};

use super::from_row::{self, FromRow};
use super::types::WebhookConfig;
use super::Database;

impl Database {
    /// Get all webhook configs for a project.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_webhook_configs(&self, project_id: i64) -> Result<Vec<WebhookConfig>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn
                .prepare("SELECT id, project_id, url, events, secret, enabled, created_at FROM webhook_configs WHERE project_id = ?1 ORDER BY created_at")
                ?;
            from_row::query_vec::<WebhookConfig>(&mut stmt, &[&project_id])
        })?
    }

    /// Get one webhook config by ID.
    #[tracing::instrument(skip(self), fields(id))]
    pub fn get_webhook_config(&self, id: i64) -> Result<Option<WebhookConfig>, DbError> {
        self.execute(move |conn| {
            conn.query_row(
                "SELECT id, project_id, url, events, secret, enabled, created_at FROM webhook_configs WHERE id = ?1",
                [id],
                WebhookConfig::from_row,
            )
            .optional()
            .map_err(DbError::from)
        })?
    }

    /// Get all webhook configs, used for credential migration.
    #[tracing::instrument(skip(self))]
    pub fn get_all_webhook_configs(&self) -> Result<Vec<WebhookConfig>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn
                .prepare("SELECT id, project_id, url, events, secret, enabled, created_at FROM webhook_configs ORDER BY project_id, created_at")
                ?;
            from_row::query_vec::<WebhookConfig>(&mut stmt, &[])
        })?
    }

    /// Save (upsert) a webhook config.
    #[tracing::instrument(skip(self, secret, url), fields(project_id, events = %events, has_secret = secret.is_some_and(|s| !s.is_empty()), enabled))]
    pub fn save_webhook_config(
        &self,
        project_id: i64,
        url: &str,
        events: &str,
        secret: Option<&str>,
        enabled: bool,
    ) -> Result<i64, DbError> {
        let url = url.to_string();
        let events = events.to_string();
        let secret = secret.map(|s| s.to_string());
        self.execute(move |conn| {
            conn.execute(
                "INSERT INTO webhook_configs (project_id, url, events, secret, enabled)
                 VALUES (:project_id, :url, :events, :secret, :enabled)
                 ON CONFLICT(project_id, url) DO UPDATE SET events=:events, secret=:secret, enabled=:enabled",
                named_params! {
                    ":project_id": project_id,
                    ":url": url,
                    ":events": events,
                    ":secret": secret,
                    ":enabled": enabled as i32,
                },
            )?;
            conn.query_row(
                "SELECT id FROM webhook_configs WHERE project_id = ?1 AND url = ?2",
                (&project_id, &url),
                |row| row.get::<_, i64>(0),
            )
            .map_err(DbError::from)
        })?
    }

    /// Remove a plaintext webhook secret from SQLite after moving it to keyring.
    #[tracing::instrument(skip(self), fields(id))]
    pub fn clear_webhook_secret(&self, id: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute(
                "UPDATE webhook_configs SET secret = NULL WHERE id = ?1",
                [id],
            )?;
            Ok(())
        })?
    }

    /// Delete a webhook config by ID.
    #[tracing::instrument(skip(self), fields(id))]
    pub fn delete_webhook_config(&self, id: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute("DELETE FROM webhook_configs WHERE id = ?1", [id])?;
            Ok(())
        })?
    }
}
