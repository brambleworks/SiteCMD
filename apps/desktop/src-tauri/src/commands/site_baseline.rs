//! Commands for viewing and deciding verified-good baseline changes.
//!
//! Accepting changes the baseline; dismissing only hides the current change.
//! Revision and digest guards reject decisions made against stale values.

use crate::connected_baseline::{ConnectedBaselineField, ConnectedBaselineProfile};
use crate::db::{BaselineDecision, BaselineDecisionOutcome, Database};
use serde::Serialize;
use sitecmd_engine::profile::{FieldValue, ProfileField, RecordOrigin, VerifiedGoodProfile};
use std::sync::Arc;
use tauri::{AppHandle, State};
use ts_rs::TS;

use super::{connected_setup::connected_client, run_blocking, sanitize_error};

// The three DTOs below use `//` comments, not doc comments: ts-rs copies `///`
// into the generated bundle, and the rationale belongs to the Rust reader.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct SiteBaseline {
    // The basis a decision must be taken against.
    pub revision: i64,
    pub fields: Vec<SiteBaselineField>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct SiteBaselineField {
    // Stored key named by decisions.
    pub field: String,
    pub label: String,
    // `changed`, `silenced`, or `good`.
    pub status: String,
    // Origin of the accepted value.
    pub origin: String,
    pub recorded_at: i64,
    // Accepted and differing facts, one per line.
    pub good_lines: Vec<String>,
    pub changed_lines: Vec<String>,
    // Digest required by a decision, or empty when no decision is pending.
    pub change_digest: String,
    pub change_first_seen_at: i64,
    pub can_dismiss: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct BaselineDecisionResult {
    pub applied: bool,
    // The refusal code when `applied` is false (`stale_revision`, `no_drift`),
    // empty otherwise, with the sentence to show beside it.
    pub refusal: String,
    pub message: String,
    // The current revision either way, so a refused view can refresh itself.
    pub revision: i64,
}

/// The site's baseline, or an empty one when nothing has been observed.
#[tracing::instrument(skip(app, db, environment_scope_key), fields(site_id))]
pub async fn get_site_baseline(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    site_id: i64,
    project_id: Option<i64>,
    environment_scope_key: Option<String>,
) -> Result<SiteBaseline, String> {
    if let (Some(project_id), Some(environment_scope_key)) = (project_id, environment_scope_key) {
        let db_read = Arc::clone(&db);
        let env_read = environment_scope_key.clone();
        let connected = run_blocking(move || db_read.get_connected_site(project_id, &env_read))
            .await?
            .map_err(sanitize_error)?
            .is_some();
        if connected {
            let (client, remote_site_id) =
                connected_client(&app, &db, project_id, environment_scope_key).await?;
            let profile = client
                .verified_good_profile(&remote_site_id)
                .await
                .map_err(sanitize_error)?;
            return Ok(render_connected_baseline(&profile));
        }
    }
    let db = (*db).clone();
    let profile = run_blocking(move || db.get_verified_good_profile(site_id))
        .await?
        .map_err(sanitize_error)?;
    Ok(render_baseline(&profile))
}

/// Accept the current change as the baseline or dismiss it without rebaselining.
#[tracing::instrument(
    skip(app, db, expected_digest, environment_scope_key),
    fields(site_id, field, accept)
)]
pub async fn decide_site_baseline(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    site_id: i64,
    field: String,
    based_on_revision: i64,
    expected_digest: String,
    accept: bool,
    project_id: Option<i64>,
    environment_scope_key: Option<String>,
) -> Result<BaselineDecisionResult, String> {
    if let (Some(project_id), Some(environment_scope_key)) = (project_id, environment_scope_key) {
        let db_read = Arc::clone(&db);
        let env_read = environment_scope_key.clone();
        let connected = run_blocking(move || db_read.get_connected_site(project_id, &env_read))
            .await?
            .map_err(sanitize_error)?
            .is_some();
        if connected {
            if !accept {
                return Ok(BaselineDecisionResult {
                    applied: false,
                    refusal: "connected_dismissal_unavailable".into(),
                    message: "Dismiss the corresponding connected finding instead; dismissing never rewrites the hosted baseline.".into(),
                    revision: based_on_revision,
                });
            }
            let (client, remote_site_id) =
                connected_client(&app, &db, project_id, environment_scope_key).await?;
            let idempotency_key = baseline_idempotency_key()?;
            return match client
                .accept_verified_good(
                    &remote_site_id,
                    &field,
                    based_on_revision,
                    &expected_digest,
                    &idempotency_key,
                )
                .await
            {
                Ok(accepted) => Ok(BaselineDecisionResult {
                    applied: true,
                    refusal: String::new(),
                    message: String::new(),
                    revision: accepted.profile_revision,
                }),
                Err(error) if error.is_stale_revision() => {
                    let current = client
                        .verified_good_profile(&remote_site_id)
                        .await
                        .map_err(sanitize_error)?;
                    Ok(BaselineDecisionResult {
                        applied: false,
                        refusal: "stale_revision".into(),
                        message: error.message,
                        revision: current.profile_revision,
                    })
                }
                Err(error) => Err(sanitize_error(error)),
            };
        }
    }
    let Some(field) = ProfileField::parse(&field) else {
        return Err("Unknown baseline family.".to_string());
    };
    let decision = if accept {
        BaselineDecision::Accept
    } else {
        BaselineDecision::Dismiss
    };
    let decided_at = chrono::Utc::now();
    let db = (*db).clone();
    let outcome = run_blocking(move || {
        let outcome = db.decide_verified_good(
            site_id,
            field,
            based_on_revision.max(0) as u64,
            expected_digest,
            decision,
            decided_at,
        )?;
        let revision = db.get_verified_good_profile(site_id)?.revision as i64;
        Ok::<_, crate::db::DbError>((outcome, revision))
    })
    .await?
    .map_err(sanitize_error)?;

    let (outcome, revision) = outcome;
    Ok(match outcome {
        BaselineDecisionOutcome::Applied { revision } => BaselineDecisionResult {
            applied: true,
            refusal: String::new(),
            message: String::new(),
            revision,
        },
        BaselineDecisionOutcome::Refused(error) => BaselineDecisionResult {
            applied: false,
            refusal: error.code().to_string(),
            message: error.message(),
            revision,
        },
    })
}

fn render_baseline(profile: &VerifiedGoodProfile) -> SiteBaseline {
    let fields = ProfileField::ALL
        .iter()
        .filter_map(|field| {
            let state = profile.fields.get(field)?;
            let drift = state.drift.as_ref();
            Some(SiteBaselineField {
                field: field.as_str().to_string(),
                label: field.label().to_string(),
                status: match drift {
                    Some(drift) if drift.dismissed => "silenced".into(),
                    Some(_) => "changed".into(),
                    None => "good".into(),
                },
                origin: origin_label(state.good.origin).to_string(),
                recorded_at: state.good.recorded_at.timestamp_millis(),
                good_lines: describe(&state.good.value),
                changed_lines: drift
                    .map(|drift| describe(&drift.value))
                    .unwrap_or_default(),
                change_digest: drift.map(|drift| drift.digest.clone()).unwrap_or_default(),
                change_first_seen_at: drift
                    .map(|drift| drift.first_seen_at.timestamp_millis())
                    .unwrap_or_default(),
                can_dismiss: true,
            })
        })
        .collect();
    SiteBaseline {
        revision: profile.revision as i64,
        fields,
    }
}

fn render_connected_baseline(profile: &ConnectedBaselineProfile) -> SiteBaseline {
    let fields = profile
        .fields
        .iter()
        .filter(|field| field.good_digest.is_some())
        .map(|field| SiteBaselineField {
            field: field.field.clone(),
            label: connected_field_label(&field.field).to_string(),
            status: if field.frozen { "changed" } else { "good" }.into(),
            origin: connected_origin_label(field.good_origin.as_deref()).into(),
            recorded_at: connected_recorded_at(field),
            good_lines: vec!["Verified by an accepted hosted observation".into()],
            changed_lines: if field.frozen {
                vec!["The latest complete hosted observation differs".into()]
            } else {
                Vec::new()
            },
            change_digest: if field.frozen {
                field.observed_digest.clone().unwrap_or_default()
            } else {
                String::new()
            },
            change_first_seen_at: connected_change_first_seen_at(field),
            can_dismiss: false,
        })
        .collect();
    SiteBaseline {
        revision: profile.profile_revision,
        fields,
    }
}

fn connected_timestamp(value: Option<&str>) -> i64 {
    value
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp_millis())
        .unwrap_or_default()
}

fn connected_recorded_at(field: &ConnectedBaselineField) -> i64 {
    let preferred = if field.good_origin.as_deref() == Some("accepted") {
        field.accepted_at.as_deref()
    } else {
        field.recorded_at.as_deref()
    };
    connected_timestamp(preferred.or_else(|| {
        field
            .observed_source
            .as_ref()
            .and_then(|source| source.observed_at.as_deref())
    }))
}

fn connected_change_first_seen_at(field: &ConnectedBaselineField) -> i64 {
    connected_timestamp(field.drift_first_seen_at.as_deref().or_else(|| {
        field
            .observed_source
            .as_ref()
            .and_then(|source| source.observed_at.as_deref())
    }))
}

fn connected_field_label(field: &str) -> &'static str {
    match field {
        "certificate" => "Certificate identity",
        "security_headers" => "Security headers",
        "third_party_origins" => "Third-party origins",
        "dns_posture" => "DNS posture",
        "route_set" => "Known routes",
        _ => "Hosted baseline",
    }
}

fn connected_origin_label(origin: Option<&str>) -> &'static str {
    match origin {
        Some("accepted") => "Accepted as the hosted baseline",
        Some("promoted") => "Re-established by a hosted scan",
        Some("reseeded") => "Re-recorded after a detector change",
        _ => "Recorded by a clean hosted scan",
    }
}

fn baseline_idempotency_key() -> Result<String, String> {
    let mut bytes = [0_u8; 24];
    getrandom::fill(&mut bytes).map_err(|error| format!("OS RNG unavailable: {error}"))?;
    Ok(format!("desktop_baseline_{}", hex::encode(bytes)))
}

fn origin_label(origin: RecordOrigin) -> &'static str {
    match origin {
        RecordOrigin::Seeded => "Recorded from the first scan that saw it",
        RecordOrigin::Promoted => "Re-established when the site came back to it",
        RecordOrigin::Accepted => "Accepted as the baseline",
        RecordOrigin::Reseeded => "Re-recorded after a detector change",
    }
}

/// A value as lines a person can compare at a glance. Deliberately flat: two
/// nested structures side by side are a diff nobody reads.
fn describe(value: &FieldValue) -> Vec<String> {
    match value {
        FieldValue::Certificate(identity) => {
            let mut lines = Vec::new();
            if let Some(issuer) = identity.issuer.as_deref() {
                lines.push(format!("Issued by {issuer}"));
            }
            lines.extend(
                identity
                    .subject_names
                    .values
                    .iter()
                    .map(|name| format!("Covers {name}")),
            );
            with_overflow(lines, identity.subject_names.overflow, "more names")
        }
        FieldValue::SecurityHeaders(profile) => profile
            .headers
            .iter()
            .flat_map(|(name, lines)| {
                lines
                    .iter()
                    .map(move |line| format!("{name}: {}", line.value))
            })
            .collect(),
        FieldValue::ThirdPartyOrigins(set) => with_overflow(
            set.origins.values.clone(),
            set.origins.overflow,
            "more origins",
        ),
        FieldValue::DnsPosture(posture) => {
            let mut lines = Vec::new();
            for host in &posture.mx_hosts.values {
                lines.push(format!("Mail exchange {host}"));
            }
            if let Some(target) = posture.cname_target.as_deref() {
                lines.push(format!("www points at {target}"));
            }
            lines.push(
                if posture.caa_present {
                    "Certificate authority records present"
                } else {
                    "No certificate authority records"
                }
                .to_string(),
            );
            if let Some(spf) = posture.spf.as_deref() {
                lines.push(format!("SPF {spf}"));
            }
            if let Some(dmarc) = posture.dmarc.as_deref() {
                lines.push(format!("DMARC {dmarc}"));
            }
            with_overflow(lines, posture.mx_hosts.overflow, "more mail exchanges")
        }
        FieldValue::RouteSet(routes) => with_overflow(
            routes.routes.values.clone(),
            routes.routes.overflow,
            "more routes",
        ),
    }
}

/// A bounded value never renders as though it were complete.
fn with_overflow(mut lines: Vec<String>, overflow: usize, noun: &str) -> Vec<String> {
    if overflow > 0 {
        lines.push(format!("and {overflow} {noun}"));
    }
    lines
}

#[cfg(test)]
#[path = "site_baseline_tests.rs"]
mod tests;
