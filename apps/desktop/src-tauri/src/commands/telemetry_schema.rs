use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_EVENTS_PER_REQUEST: usize = 20;
const MAX_PROPERTY_COUNT: usize = 40;
const MAX_TEXT_LENGTH: usize = 240;
const MAX_DIAGNOSTIC_PROPERTIES: usize = 16;
const MAX_DIAGNOSTIC_STACK_LINES: usize = 40;
const MAX_DIAGNOSTIC_STACK_LINE_LENGTH: usize = 500;

static IDENTIFIER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z]+_[a-zA-Z0-9._-]{8,120}$").expect("valid regex") // allow-expect: literal
});
static SHA256_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-f0-9]{64}$").expect("valid telemetry sha256 regex") // allow-expect: compile-time regex literal
});
static DELETE_SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^delete_[a-zA-Z0-9._-]{32,200}$").expect("valid regex") // allow-expect: literal
});
static APP_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{1,5}\.\d{1,5}\.\d{1,5}(?:[-+][0-9A-Za-z.-]{1,32})?$")
        .expect("valid app version regex") // allow-expect: compile-time regex literal
});
static BUILD_CHANNEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z][a-z0-9-]{0,31}$").expect("valid build channel regex") // allow-expect: compile-time regex literal
});
static PROPERTY_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z][a-zA-Z0-9_]{0,40}$").expect("valid regex") // allow-expect: literal
});
static FORBIDDEN_PROPERTY_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:url|uri|path|email|token|secret|password|license|api[_-]?key|webhook|source|code|body|payload|project[_-]?name|site[_-]?name|id$)",
    )
    .expect("valid forbidden telemetry property key regex") // allow-expect: compile-time regex literal
});
static SENSITIVE_PROPERTY_TEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:https?://[^\s)]+|\b(?:localhost|127\.0\.0\.1)(?::\d+)?\b|/(?:Users|home|var|tmp|private|Volumes)/[^\s)]+|[A-Z]:\\[^\s)]+|[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}|\b(?:ghp|github_pat|sk|rk|pk|xox[baprs]|AIza)[A-Za-z0-9_:-]{8,}\b|\b(?:authorization\s*:\s*bearer\s+|api[_-]?key\s*[:=]\s*|token\s*[:=]\s*|secret\s*[:=]\s*|license[_-]?key\s*[:=]\s*)["']?[^"',;\s]+)"#,
    )
    .expect("valid sensitive telemetry text regex") // allow-expect: compile-time regex literal
});

const EVENT_NAMES: &[&str] = &[
    "workflow_event",
    "telemetry_consent_saved",
    "telemetry_preview_opened",
    "telemetry_uploaded_deletion_requested",
];
const WORKFLOW_NAMES: &[&str] = &[
    "add_site",
    "run_scan",
    "open_issues",
    "copy_guidance",
    "verify_issue",
];
const WORKFLOW_STATUSES: &[&str] = &["started", "succeeded", "failed"];
const WORKFLOW_PROPERTY_KEYS: &[&str] = &[
    "workflowName",
    "workflowStatus",
    "kind",
    "mode",
    "scanMode",
    "scanType",
    "scanOutcome",
    "durationBucket",
    "durationMs",
    "totalIssues",
    "issueCount",
    "criticalIssues",
    "highIssues",
    "mediumIssues",
    "lowIssues",
    "confirmedIssues",
    "highConfidenceIssues",
    "needsReviewIssues",
    "accessibilityIssues",
    "aiSafetyIssues",
    "architectureIssues",
    "complianceIssues",
    "configIssues",
    "databaseIssues",
    "dependencyIssues",
    "performanceIssues",
    "polishIssues",
    "reliabilityIssues",
    "securityIssues",
    "seoIssues",
    "pageCount",
    "completedPages",
    "environmentCount",
    "primaryEnvironment",
    "hasFolder",
    "hasError",
    "errorType",
    "surface",
    "category",
    "beforeIssueCount",
    "afterIssueCount",
    "executionStatus",
    "reused",
];
const BOOLEAN_WORKFLOW_KEYS: &[&str] = &["hasFolder", "hasError", "reused"];
const DIAGNOSTIC_PROPERTY_KEYS: &[&str] = &[
    "boundary",
    "brokered",
    "command",
    "component",
    "fatal",
    "kind",
    "page",
    "phase",
    "source",
    "status",
    "surface",
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryRegistrationBody {
    pub(super) subject_id: String,
    pub(super) delete_proof_hash: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryDeletionBody {
    pub(super) subject_id: String,
    pub(super) delete_secret: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryIngestBody {
    pub(super) events: Vec<TelemetryEvent>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct TelemetryEvent {
    schema_version: u8,
    id: String,
    kind: String,
    name: String,
    occurred_at: String,
    app_version: String,
    build_channel: String,
    os_family: String,
    architecture: String,
    tier: String,
    anonymous_subject_id: String,
    delete_proof_hash: String,
    consent_version: u32,
    properties: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticReport {
    pub(super) name: String,
    pub(super) message: String,
    pub(super) stack: Option<String>,
    #[serde(default)]
    pub(super) properties: BTreeMap<String, Value>,
    pub(super) app_version: String,
    pub(super) build_channel: String,
}

pub(super) fn validate_registration(body: &TelemetryRegistrationBody) -> Result<(), String> {
    validate_identifier(&body.subject_id, "Telemetry subject ID")?;
    validate_sha256(&body.delete_proof_hash, "Telemetry deletion proof")
}

pub(super) fn validate_deletion(body: &TelemetryDeletionBody) -> Result<(), String> {
    validate_identifier(&body.subject_id, "Telemetry subject ID")?;
    if !DELETE_SECRET_RE.is_match(&body.delete_secret) {
        return Err("Telemetry deletion secret is invalid".to_string());
    }
    Ok(())
}

pub(super) fn validate_ingest(body: &TelemetryIngestBody) -> Result<(), String> {
    if body.events.is_empty() || body.events.len() > MAX_EVENTS_PER_REQUEST {
        return Err("Telemetry event batch size is invalid".to_string());
    }
    for event in &body.events {
        validate_event(event)?;
    }
    Ok(())
}

pub(super) fn sanitized_diagnostic(report: &DiagnosticReport) -> Result<Value, String> {
    if !matches!(
        report.name.as_str(),
        "frontend_error" | "tauri_command_failed" | "startup_error"
    ) {
        return Err("Diagnostic event name is not allowed".to_string());
    }
    validate_app_version(&report.app_version)?;
    validate_build_channel(&report.build_channel)?;

    if report.message.is_empty()
        || report.message.len() > crate::constants::TELEMETRY_REQUEST_MAX_BYTES
    {
        return Err("Diagnostic message is invalid".to_string());
    }
    let message = crate::log_sanitizer::bounded_issue_evidence(&report.message);

    let properties = sanitize_diagnostic_properties(&report.properties)?;
    let stack = report.stack.as_deref().map(sanitize_stack).transpose()?;
    Ok(serde_json::json!({
        "name": report.name,
        "message": message,
        "stack": stack,
        "properties": properties,
        "appVersion": report.app_version,
        "buildChannel": report.build_channel,
    }))
}

fn validate_event(event: &TelemetryEvent) -> Result<(), String> {
    if event.schema_version != 1
        || event.kind != "usage"
        || !EVENT_NAMES.contains(&event.name.as_str())
    {
        return Err("Telemetry event identity is invalid".to_string());
    }
    validate_identifier(&event.id, "Telemetry event ID")?;
    validate_identifier(&event.anonymous_subject_id, "Telemetry subject ID")?;
    validate_sha256(&event.delete_proof_hash, "Telemetry deletion proof")?;
    validate_app_version(&event.app_version)?;
    validate_build_channel(&event.build_channel)?;
    validate_timestamp(&event.occurred_at)?;
    if event.consent_version == 0 {
        return Err("Telemetry consent version is invalid".to_string());
    }
    if !matches!(
        event.os_family.as_str(),
        "macos" | "windows" | "linux" | "unknown"
    ) || !matches!(
        event.architecture.as_str(),
        "arm64" | "aarch64" | "x86_64" | "x86" | "unknown"
    ) || !matches!(event.tier.as_str(), "free" | "core" | "pro" | "unknown")
    {
        return Err("Telemetry device descriptor is invalid".to_string());
    }
    validate_properties(&event.name, &event.properties)
}

fn validate_properties(name: &str, properties: &BTreeMap<String, Value>) -> Result<(), String> {
    if properties.len() > MAX_PROPERTY_COUNT {
        return Err("Telemetry property count is invalid".to_string());
    }

    let allowed: BTreeSet<&str> = match name {
        "workflow_event" => WORKFLOW_PROPERTY_KEYS.iter().copied().collect(),
        "telemetry_consent_saved" => ["usageAnalytics", "crashReports"].into_iter().collect(),
        _ => BTreeSet::new(),
    };
    for (key, value) in properties {
        if !PROPERTY_KEY_RE.is_match(key)
            || FORBIDDEN_PROPERTY_KEY_RE.is_match(key)
            || !allowed.contains(key.as_str())
        {
            return Err("Telemetry property key is not allowed".to_string());
        }
        validate_property_value(name, key, value)?;
    }

    if name == "workflow_event" {
        let workflow_name = properties.get("workflowName").and_then(Value::as_str);
        let workflow_status = properties.get("workflowStatus").and_then(Value::as_str);
        if !workflow_name.is_some_and(|value| WORKFLOW_NAMES.contains(&value))
            || !workflow_status.is_some_and(|value| WORKFLOW_STATUSES.contains(&value))
        {
            return Err("Telemetry workflow identity is invalid".to_string());
        }
    } else if name == "telemetry_consent_saved"
        && (!properties
            .get("usageAnalytics")
            .is_some_and(Value::is_boolean)
            || !properties
                .get("crashReports")
                .is_some_and(Value::is_boolean))
    {
        return Err("Telemetry consent event is invalid".to_string());
    }
    Ok(())
}

fn validate_property_value(name: &str, key: &str, value: &Value) -> Result<(), String> {
    let is_number_key = key.ends_with("Issues")
        || key.ends_with("Count")
        || matches!(key, "durationMs" | "pageCount" | "completedPages");
    let valid = if name == "telemetry_consent_saved" || BOOLEAN_WORKFLOW_KEYS.contains(&key) {
        value.is_boolean()
    } else if name == "workflow_event" && is_number_key {
        value
            .as_f64()
            .is_some_and(|number| number.is_finite() && (0.0..=1_000_000_000.0).contains(&number))
    } else {
        value.is_null()
            || value.as_str().is_some_and(|text| {
                text.chars().count() <= MAX_TEXT_LENGTH
                    && !SENSITIVE_PROPERTY_TEXT_RE.is_match(text)
                    && crate::log_sanitizer::bounded_issue_evidence(text) == text
            })
    };
    if valid {
        Ok(())
    } else {
        Err("Telemetry property value is invalid".to_string())
    }
}

fn sanitize_diagnostic_properties(
    properties: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    if properties.len() > MAX_DIAGNOSTIC_PROPERTIES {
        return Err("Diagnostic property count is invalid".to_string());
    }
    let allowed: BTreeSet<&str> = DIAGNOSTIC_PROPERTY_KEYS.iter().copied().collect();
    let mut sanitized = BTreeMap::new();
    for (key, value) in properties {
        if !allowed.contains(key.as_str()) || FORBIDDEN_PROPERTY_KEY_RE.is_match(key) {
            continue;
        }
        let value = match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
            Value::String(text) => {
                Value::String(crate::log_sanitizer::bounded_issue_evidence(text))
            }
            _ => continue,
        };
        sanitized.insert(key.clone(), value);
    }
    Ok(sanitized)
}

fn sanitize_stack(stack: &str) -> Result<Vec<String>, String> {
    if stack.len() > crate::constants::TELEMETRY_REQUEST_MAX_BYTES {
        return Err("Diagnostic stack is too large".to_string());
    }
    Ok(stack
        .lines()
        .take(MAX_DIAGNOSTIC_STACK_LINES)
        .map(|line| {
            crate::log_sanitizer::bounded_issue_evidence(
                &line
                    .chars()
                    .take(MAX_DIAGNOSTIC_STACK_LINE_LENGTH)
                    .collect::<String>(),
            )
        })
        .collect())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if IDENTIFIER_RE.is_match(value) {
        Ok(())
    } else {
        Err(format!("{label} is invalid"))
    }
}

fn validate_sha256(value: &str, label: &str) -> Result<(), String> {
    if SHA256_RE.is_match(value) {
        Ok(())
    } else {
        Err(format!("{label} is invalid"))
    }
}

fn validate_app_version(value: &str) -> Result<(), String> {
    if APP_VERSION_RE.is_match(value) {
        Ok(())
    } else {
        Err("Telemetry app version is invalid".to_string())
    }
}

fn validate_build_channel(value: &str) -> Result<(), String> {
    if BUILD_CHANNEL_RE.is_match(value) {
        Ok(())
    } else {
        Err("Telemetry build channel is invalid".to_string())
    }
}

fn validate_timestamp(value: &str) -> Result<(), String> {
    let timestamp = DateTime::parse_from_rfc3339(value)
        .map_err(|_| "Telemetry timestamp is invalid".to_string())?
        .with_timezone(&Utc);
    let now = Utc::now();
    if timestamp < now - chrono::Duration::days(7)
        || timestamp > now + chrono::Duration::minutes(10)
    {
        return Err("Telemetry timestamp is outside the accepted window".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_event() -> TelemetryEvent {
        TelemetryEvent {
            schema_version: 1,
            id: "event_12345678".to_string(),
            kind: "usage".to_string(),
            name: "workflow_event".to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            app_version: "1.2.3".to_string(),
            build_channel: "production".to_string(),
            os_family: "macos".to_string(),
            architecture: "arm64".to_string(),
            tier: "free".to_string(),
            anonymous_subject_id: "scmd_12345678".to_string(),
            delete_proof_hash: "a".repeat(64),
            consent_version: 1,
            properties: BTreeMap::from([
                (
                    "workflowName".to_string(),
                    Value::String("run_scan".to_string()),
                ),
                (
                    "workflowStatus".to_string(),
                    Value::String("succeeded".to_string()),
                ),
            ]),
        }
    }

    #[test]
    fn usage_schema_rejects_unknown_or_sensitive_properties() {
        let mut event = valid_event();
        assert!(validate_event(&event).is_ok());
        event.properties.insert(
            "sourceCode".to_string(),
            Value::String("secret".to_string()),
        );
        assert!(validate_event(&event).is_err());
    }

    #[test]
    fn usage_schema_rejects_identifiers_and_unscrubbed_text() {
        let mut identifier_event = valid_event();
        identifier_event
            .properties
            .insert("executionId".to_string(), Value::Number(42.into()));
        assert!(validate_event(&identifier_event).is_err());

        let mut path_event = valid_event();
        path_event.properties.insert(
            "surface".to_string(),
            Value::String("/Users/dev/private-project".to_string()),
        );
        assert!(validate_event(&path_event).is_err());
    }

    #[test]
    fn diagnostic_schema_drops_unapproved_metadata_and_scrubs_strings() {
        let report = DiagnosticReport {
            name: "frontend_error".to_string(),
            message: "Failed at https://example.com/?token=secret".to_string(),
            stack: Some("at /Users/dev/project/main.ts:1".to_string()),
            properties: BTreeMap::from([
                ("page".to_string(), Value::String("dashboard".to_string())),
                (
                    "sourceCode".to_string(),
                    Value::String("const secret = true".to_string()),
                ),
            ]),
            app_version: "1.2.3".to_string(),
            build_channel: "production".to_string(),
        };
        let value = sanitized_diagnostic(&report).expect("valid diagnostic");
        assert_eq!(value["properties"]["page"], "dashboard");
        assert!(value["properties"].get("sourceCode").is_none());
        assert!(!value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("token=secret"));
    }
}
