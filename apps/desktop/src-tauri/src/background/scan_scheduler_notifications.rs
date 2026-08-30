//! Native notification copy for scheduled scan regressions.

use tauri::AppHandle;

use crate::core::scanner::ScanType;

pub(super) fn send_full_scan_notification(
    app_handle: &AppHandle,
    url: &str,
    score: u32,
    issue_count: usize,
    critical: usize,
) {
    use tauri_plugin_notification::NotificationExt;

    let hostname = hostname_for_url(url);
    let body = if critical > 0 {
        format!(
            "{} scheduled full scan complete. SiteCMD Score {}/100 - {} critical issue{} among {} tracked.",
            hostname,
            score,
            critical,
            plural_suffix(critical),
            issue_count
        )
    } else {
        format!(
            "{} scheduled full scan complete. SiteCMD Score {}/100 - {} issue{} tracked.",
            hostname,
            score,
            issue_count,
            plural_suffix(issue_count)
        )
    };

    if let Err(error) = app_handle
        .notification()
        .builder()
        .title("SiteCMD - Scheduled Full Scan")
        .body(&body)
        .show()
    {
        tracing::warn!("Failed to send notification: {:?}", error);
    } else {
        tracing::info!("Notification sent: {}", body);
    }
}

pub(super) fn send_code_scan_notification(
    app_handle: &AppHandle,
    url: &str,
    prev_score: Option<u32>,
    new_score: u32,
    new_critical: usize,
    issue_count: usize,
    domain_trend_label: Option<&str>,
) {
    use tauri_plugin_notification::NotificationExt;

    let hostname = hostname_for_url(url);
    let domain_trend_suffix = domain_trend_label
        .map(|label| format!(" {label}."))
        .unwrap_or_default();
    let body = match prev_score {
        Some(prev) if new_critical > 0 => format!(
            "{} scheduled Code Scan diagnostic dropped from {} to {}. {} critical code issue{} found.{}",
            hostname,
            prev,
            new_score,
            new_critical,
            plural_suffix(new_critical),
            domain_trend_suffix
        ),
        Some(prev) => format!(
            "{} scheduled Code Scan diagnostic dropped from {} to {}. {} code issue{} detected.{}",
            hostname,
            prev,
            new_score,
            issue_count,
            plural_suffix(issue_count),
            domain_trend_suffix
        ),
        None => format!(
            "{} scheduled Code Scan found {} critical code issue{} (diagnostic: {}/100).{}",
            hostname,
            new_critical,
            plural_suffix(new_critical),
            new_score,
            domain_trend_suffix
        ),
    };

    if let Err(error) = app_handle
        .notification()
        .builder()
        .title("SiteCMD - Scheduled Code Alert")
        .body(&body)
        .show()
    {
        tracing::warn!("Failed to send notification: {:?}", error);
    } else {
        tracing::info!("Notification sent: {}", body);
    }
}

pub(super) fn send_web_scan_notification(
    app_handle: &AppHandle,
    url: &str,
    scan_type: ScanType,
    prev_score: Option<u32>,
    new_score: u32,
    new_critical: usize,
    issue_count: usize,
) {
    use tauri_plugin_notification::NotificationExt;

    let hostname = hostname_for_url(url);
    let scan_label = if scan_type == ScanType::Security {
        "Security Scan"
    } else {
        "Web Scan"
    };
    let body = if let Some(prev) = prev_score {
        if new_critical > 0 {
            format!(
                "{} scheduled {} diagnostic dropped from {} to {}. {} critical issue{} found.",
                hostname,
                scan_label,
                prev,
                new_score,
                new_critical,
                plural_suffix(new_critical)
            )
        } else {
            format!(
                "{} scheduled {} diagnostic dropped from {} to {}. {} actionable issue{} detected.",
                hostname,
                scan_label,
                prev,
                new_score,
                issue_count,
                plural_suffix(issue_count)
            )
        }
    } else {
        format!(
            "{} scheduled {} found {} critical issue{} (diagnostic: {}/100).",
            hostname,
            scan_label,
            new_critical,
            plural_suffix(new_critical),
            new_score
        )
    };
    let title = if scan_type == ScanType::Security {
        "SiteCMD - Scheduled Security Alert"
    } else {
        "SiteCMD - Scheduled Web Alert"
    };

    if let Err(error) = app_handle
        .notification()
        .builder()
        .title(title)
        .body(&body)
        .show()
    {
        tracing::warn!("Failed to send notification: {:?}", error);
    } else {
        tracing::info!("Notification sent: {}", body);
    }
}

pub(super) fn hostname_for_url(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(String::from))
        .unwrap_or_else(|| url.to_string())
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}
