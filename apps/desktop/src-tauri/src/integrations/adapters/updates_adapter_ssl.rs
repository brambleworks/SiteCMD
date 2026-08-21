//! Builds SSL certificate-expiry alerts for the Updates adapter.

use crate::db::alerts::AlertInput;
use crate::updates::ssl::SslCertInfo;

pub(super) fn build_ssl_expiry_alert(
    project_id: i64,
    env_url: &str,
    cert: &SslCertInfo,
    observed_at: i64,
) -> Option<AlertInput> {
    if cert.days_until_expiry >= 30 {
        return None;
    }

    let alert_severity = if cert.days_until_expiry < 7 {
        "critical"
    } else {
        "warn"
    };
    let expiry_day = cert.not_after.format("%Y-%m-%d").to_string();

    Some(AlertInput {
        project_id,
        env_url: Some(env_url.to_string()),
        source: "updates".to_string(),
        alert_id: format!(
            "ssl-expiring:{}:{}:{}",
            cert.host, expiry_day, alert_severity
        ),
        severity: alert_severity.to_string(),
        title: ssl_expiry_title(cert.days_until_expiry),
        description: ssl_expiry_description(&cert.host, cert.days_until_expiry, &expiry_day),
        detail_json: Some(
            serde_json::json!({
                "alert_type": "ssl_expiring",
                "host": cert.host,
                "days_until_expiry": cert.days_until_expiry,
                "not_after": cert.not_after,
                "url": env_url,
                "destination": "security"
            })
            .to_string(),
        ),
        occurred_at: observed_at,
        observed_at,
    })
}

pub(super) fn ssl_expiry_title(days_until_expiry: i64) -> String {
    if days_until_expiry < 0 {
        let days_expired = days_until_expiry.abs();
        return format!(
            "SSL certificate expired {} {} ago",
            days_expired,
            if days_expired == 1 { "day" } else { "days" }
        );
    }
    if days_until_expiry == 0 {
        return "SSL certificate expires today".to_string();
    }
    format!(
        "SSL certificate expires in {} {}",
        days_until_expiry,
        if days_until_expiry == 1 {
            "day"
        } else {
            "days"
        }
    )
}

pub(super) fn ssl_expiry_description(
    host: &str,
    days_until_expiry: i64,
    expiry_day: &str,
) -> String {
    if days_until_expiry < 0 {
        return format!(
            "{host}'s certificate expired on {expiry_day}. Renew it now because visitors may already see browser warnings."
        );
    }
    if days_until_expiry == 0 {
        return format!(
            "{host}'s certificate expires today ({expiry_day}). Renew it now before visitors start seeing browser warnings."
        );
    }
    format!(
        "{host}'s certificate expires on {expiry_day}. Renew it before visitors start seeing browser warnings."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssl_expiry_alerts_only_emit_inside_thirty_days() {
        let soon = SslCertInfo {
            host: "example.com".into(),
            days_until_expiry: 6,
            not_after: chrono::DateTime::parse_from_rfc3339("2026-05-11T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        };
        let later = SslCertInfo {
            days_until_expiry: 30,
            ..soon.clone()
        };

        let alert =
            build_ssl_expiry_alert(7, "https://example.com", &soon, 1_000).expect("ssl alert");
        assert_eq!(alert.severity, "critical");
        assert_eq!(alert.env_url.as_deref(), Some("https://example.com"));
        assert!(build_ssl_expiry_alert(7, "https://example.com", &later, 1_000).is_none());
    }

    #[test]
    fn ssl_expiry_alert_handles_already_expired_certificates() {
        let expired = SslCertInfo {
            host: "example.com".into(),
            days_until_expiry: -2,
            not_after: chrono::DateTime::parse_from_rfc3339("2026-05-14T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        };

        let alert =
            build_ssl_expiry_alert(7, "https://example.com", &expired, 1_000).expect("ssl alert");

        assert_eq!(alert.title, "SSL certificate expired 2 days ago");
        assert!(alert
            .description
            .contains("visitors may already see browser warnings"));
    }
}
