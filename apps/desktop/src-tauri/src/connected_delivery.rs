//! Shareable reports and signed alert deliveries. Secrets appear only in
//! create and rotate responses.

use serde::{Deserialize, Serialize};

use crate::connected_service::{local_error, ConnectedServiceClient, ConnectedServiceError};

/// What a report shows, exactly as stored on the registry row. Route-level
/// detail is the one explicit content toggle; trends are on unless turned off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportToggles {
    #[serde(default)]
    pub include_routes: bool,
    #[serde(default = "default_true")]
    pub include_trends: bool,
    #[serde(default = "default_trend_window")]
    pub trend_window_days: u32,
}

fn default_true() -> bool {
    true
}

fn default_trend_window() -> u32 {
    30
}

#[derive(Debug, Serialize)]
struct CreateReportRequest {
    toggles: ReportToggles,
    ttl_days: u32,
}

/// The registry row and its signed link. The link is minted here and never
/// listed again, so this answer is the caller's one chance to copy it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreatedReport {
    pub report_id: String,
    pub link: String,
    pub expires_at: String,
    #[serde(default)]
    pub as_of_event_sequence: i64,
}

/// One registry row: who cut the report, when, from what, and whether the
/// link still opens. View counts are counts, never reader identities.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReportRegistryRow {
    pub report_id: String,
    pub created_at: String,
    pub created_by: String,
    pub toggles: ReportToggles,
    #[serde(default)]
    pub as_of_event_sequence: i64,
    pub expires_at: String,
    pub revoked: bool,
    #[serde(default)]
    pub revoked_at: Option<String>,
    #[serde(default)]
    pub view_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReportRegistryPage {
    #[serde(default)]
    pub items: Vec<ReportRegistryRow>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RevokedReport {
    pub report_id: String,
    pub revoked_at: String,
}

/// A created endpoint with its shown-once signing secret.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreatedAlertWebhook {
    pub webhook_id: String,
    pub url: String,
    pub secret: String,
    pub secret_fingerprint: String,
    pub secret_generation: i64,
}

/// One endpoint as listed: metadata, health, and the secret's fingerprint.
/// There is no secret field because the service cannot answer one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AlertWebhookRow {
    pub webhook_id: String,
    pub url: String,
    pub secret_fingerprint: String,
    pub secret_generation: i64,
    pub disabled: bool,
    #[serde(default)]
    pub disabled_reason: Option<String>,
    #[serde(default)]
    pub rotation_overlap_until: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct AlertWebhookList {
    #[serde(default)]
    items: Vec<AlertWebhookRow>,
}

/// A rotated endpoint: the next generation's shown-once secret and how long
/// deliveries stay signed under both generations.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RotatedAlertWebhook {
    pub webhook_id: String,
    pub secret: String,
    #[serde(default)]
    pub secret_fingerprint: String,
    pub secret_generation: i64,
    pub rotation_overlap_until: String,
}

/// The enqueued test delivery. The attempt id is what the receiver will see
/// in the X-SiteCMD-Delivery header when the queue consumer posts it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AlertWebhookTestReceipt {
    pub attempt_id: String,
    pub webhook_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeletedAlertWebhook {
    pub webhook_id: String,
}

const MAX_REPORT_PAGES: usize = 5;
const REPORT_PAGE_LIMIT: &str = "200";

impl ConnectedServiceClient {
    /// Cut a frozen report from the site's current state and receive the one
    /// copy of its capability link.
    pub async fn create_report(
        &self,
        site_id: &str,
        toggles: ReportToggles,
        ttl_days: u32,
    ) -> Result<CreatedReport, ConnectedServiceError> {
        let body = serde_json::to_string(&CreateReportRequest { toggles, ttl_days })
            .map_err(|_| serialization_refused("report"))?;
        let url = self.url(&["v1", "sites", site_id, "reports"])?;
        self.request(reqwest::Method::POST, url, None, Some(body))
            .await
    }

    /// List report registry pages up to `MAX_REPORT_PAGES`.
    pub async fn list_reports(
        &self,
        site_id: &str,
    ) -> Result<Vec<ReportRegistryRow>, ConnectedServiceError> {
        let mut rows = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_REPORT_PAGES {
            let mut url = self.url(&["v1", "sites", site_id, "reports"])?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("limit", REPORT_PAGE_LIMIT);
                if let Some(cursor) = cursor.as_deref() {
                    query.append_pair("cursor", cursor);
                }
            }
            let page: ReportRegistryPage =
                self.request(reqwest::Method::GET, url, None, None).await?;
            rows.extend(page.items);
            match page.next_cursor.filter(|value| !value.is_empty()) {
                Some(next) => cursor = Some(next),
                None => return Ok(rows),
            }
        }
        Err(local_error(
            "report_registry_overflow",
            "the connected service returned too many report pages",
        ))
    }

    /// Revoke a report link. Wins over token expiry the moment it commits,
    /// and repeating it answers the original revocation stamp.
    pub async fn revoke_report(
        &self,
        site_id: &str,
        report_id: &str,
    ) -> Result<RevokedReport, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "reports", report_id, "revoke"])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }

    /// Register an outbound alert webhook. The signing secret in the answer
    /// is derived, shown here, and never stored anywhere it could be re-read.
    pub async fn create_alert_webhook(
        &self,
        site_id: &str,
        endpoint_url: &str,
    ) -> Result<CreatedAlertWebhook, ConnectedServiceError> {
        let body = serde_json::to_string(&serde_json::json!({ "url": endpoint_url }))
            .map_err(|_| serialization_refused("webhook"))?;
        let url = self.url(&["v1", "sites", site_id, "alert-webhooks"])?;
        self.request(reqwest::Method::POST, url, None, Some(body))
            .await
    }

    pub async fn list_alert_webhooks(
        &self,
        site_id: &str,
    ) -> Result<Vec<AlertWebhookRow>, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "alert-webhooks"])?;
        let listing: AlertWebhookList = self.request(reqwest::Method::GET, url, None, None).await?;
        Ok(listing.items)
    }

    /// Enqueue a signed, explicitly-marked test delivery. A test reaches a
    /// disabled endpoint on purpose: its success is the deliberate human act
    /// that re-enables the endpoint.
    pub async fn test_alert_webhook(
        &self,
        site_id: &str,
        webhook_id: &str,
    ) -> Result<AlertWebhookTestReceipt, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "alert-webhooks", webhook_id, "test"])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }

    /// Rotate the endpoint's secret with the service's bounded dual-validity
    /// overlap, receiving the one copy of the next generation's secret.
    pub async fn rotate_alert_webhook(
        &self,
        site_id: &str,
        webhook_id: &str,
    ) -> Result<RotatedAlertWebhook, ConnectedServiceError> {
        let url = self.url(&[
            "v1",
            "sites",
            site_id,
            "alert-webhooks",
            webhook_id,
            "rotate",
        ])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }

    pub async fn delete_alert_webhook(
        &self,
        site_id: &str,
        webhook_id: &str,
    ) -> Result<DeletedAlertWebhook, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "alert-webhooks", webhook_id])?;
        self.request(reqwest::Method::DELETE, url, None, None).await
    }
}

fn serialization_refused(kind: &str) -> ConnectedServiceError {
    local_error(
        "serialization_failed",
        &format!("connected {kind} request could not be encoded"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_request_encodes_the_service_contract_shape() {
        let body = serde_json::to_string(&CreateReportRequest {
            toggles: ReportToggles {
                include_routes: true,
                include_trends: true,
                trend_window_days: 30,
            },
            ttl_days: 7,
        })
        .expect("encode");
        assert_eq!(
            body,
            r#"{"toggles":{"include_routes":true,"include_trends":true,"trend_window_days":30},"ttl_days":7}"#
        );
    }

    #[test]
    fn registry_rows_default_the_toggles_the_service_defaults() {
        // A row created before a toggle existed omits it; the desktop must
        // read the same defaults the service documents, not zero values.
        let row: ReportRegistryRow = serde_json::from_str(
            r#"{
                "report_id": "rep_1",
                "created_at": "2026-08-10T00:00:00Z",
                "created_by": "inst_a",
                "toggles": {},
                "expires_at": "2026-09-09T00:00:00Z",
                "revoked": false,
                "view_count": 3
            }"#,
        )
        .expect("parse");
        assert!(!row.toggles.include_routes);
        assert!(row.toggles.include_trends);
        assert_eq!(row.toggles.trend_window_days, 30);
        assert_eq!(row.view_count, 3);
    }

    #[test]
    fn webhook_listing_rows_carry_health_and_never_a_secret() {
        let listing: AlertWebhookList = serde_json::from_str(
            r#"{"items": [{
                "webhook_id": "awh_1",
                "url": "https://hooks.example.com/sitecmd",
                "secret_fingerprint": "sha256:0123456789abcdef",
                "secret_generation": 2,
                "disabled": true,
                "disabled_reason": "persistent_failure",
                "rotation_overlap_until": "2026-08-11T00:00:00Z",
                "created_at": "2026-08-01T00:00:00Z",
                "created_by": "inst_a",
                "secret": "must-never-be-modeled"
            }]}"#,
        )
        .expect("parse");
        let row = &listing.items[0];
        assert_eq!(row.disabled_reason.as_deref(), Some("persistent_failure"));
        let serialized = format!("{row:?}");
        assert!(!serialized.contains("must-never-be-modeled"));
    }
}
