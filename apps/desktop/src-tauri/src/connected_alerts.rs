//! Connected account alert-stream client and wire types.
//!
//! The desktop reads the newest page only and reports truncation when older
//! alerts are not reachable without a stored cursor.

use serde::Deserialize;

use crate::connected_service::{ConnectedServiceClient, ConnectedServiceError};

/// Aggregated alert cause. `None` means the event class carries no severity.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AlertCauseLine {
    pub kind: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub count: i64,
}

/// Durable delivery outcome materialized onto an alert target.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AlertDeliveryCell {
    pub target_kind: String,
    pub target_id: String,
    #[serde(default)]
    pub delivery_generation: i64,
    pub outcome: String,
}

/// Account-level alert using the optionality declared by the service contract.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AlertStreamItem {
    pub id: String,
    #[serde(default)]
    pub alert_sequence: i64,
    pub site: String,
    #[serde(default)]
    pub top_severity: Option<String>,
    #[serde(default)]
    pub causes: Vec<AlertCauseLine>,
    #[serde(default)]
    pub content_mode: Option<String>,
    #[serde(default)]
    pub deployment_id: Option<String>,
    #[serde(default)]
    pub delivery: Vec<AlertDeliveryCell>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AlertStreamPage {
    #[serde(default)]
    pub items: Vec<AlertStreamItem>,
    #[serde(default)]
    pub next_cursor: Option<String>,
    /// The account's highest claimed stream position. Account-level and
    /// unfiltered, so it is NOT the sequence of the newest alert this
    /// installation can see; it is only ever a resume-from-now cursor.
    #[serde(default)]
    pub current_sequence: i64,
}

/// Explicit alert page size, kept below the service ceiling for an interactive
/// timeline while still allowing the caller to detect a full page.
pub const ALERT_STREAM_PAGE_LIMIT: u32 = 200;

impl ConnectedServiceClient {
    /// The newest page of the account's alert stream, filtered by the service
    /// to the sites this installation is assigned.
    pub async fn list_alerts(&self, limit: u32) -> Result<AlertStreamPage, ConnectedServiceError> {
        let mut url = self.url(&["v1", "alerts"])?;
        url.query_pairs_mut()
            .append_pair("limit", &limit.to_string());
        self.request(reqwest::Method::GET, url, None, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_stream_item_parses_every_member_the_service_documents() {
        let page: AlertStreamPage = serde_json::from_str(
            r#"{
                "items": [{
                    "id": "alr_0123456789abcdef01234567",
                    "alert_sequence": 12,
                    "site": "site_a",
                    "top_severity": "critical",
                    "causes": [
                        {"kind": "regression", "severity": "critical", "count": 2},
                        {"kind": "protection_degradation", "severity": null, "count": 1}
                    ],
                    "content_mode": "private",
                    "deployment_id": "dep_9",
                    "delivery": [
                        {"target_kind": "destination", "target_id": "dst_1",
                         "delivery_generation": 2, "outcome": "sent"}
                    ],
                    "created_at": "2026-08-10T12:00:00.000Z",
                    "updated_at": "2026-08-10T12:05:00.000Z"
                }],
                "next_cursor": "12",
                "current_sequence": 40
            }"#,
        )
        .expect("parse");
        let item = &page.items[0];
        assert_eq!(item.alert_sequence, 12);
        assert_eq!(item.top_severity.as_deref(), Some("critical"));
        // A cause that bears no finding severity keeps its null rather than
        // collapsing into a severity the service never assigned.
        assert_eq!(item.causes[1].severity, None);
        assert_eq!(item.delivery[0].outcome, "sent");
        assert_eq!(page.current_sequence, 40);
    }

    #[test]
    fn an_alert_with_nothing_optional_still_parses() {
        let page: AlertStreamPage = serde_json::from_str(
            r#"{"items": [{
                "id": "alr_1", "site": "site_a", "created_at": "2026-08-10T12:00:00.000Z"
            }], "next_cursor": null}"#,
        )
        .expect("parse");
        let item = &page.items[0];
        assert!(item.delivery.is_empty());
        assert!(item.causes.is_empty());
        assert_eq!(item.updated_at, None);
        assert_eq!(item.content_mode, None);
        // No cursor and no sequence is the empty-account shape, not an error.
        assert_eq!(page.current_sequence, 0);
    }

    #[test]
    fn an_absent_items_member_reads_as_an_empty_stream() {
        let page: AlertStreamPage =
            serde_json::from_str(r#"{"current_sequence": 3}"#).expect("parse");
        assert!(page.items.is_empty());
        assert_eq!(page.next_cursor, None);
    }
}
