//! Verified-good lifecycle: recovery or acceptance advances the baseline.

mod projection;
pub use projection::*;

use projection::{is_false, Comparison};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Projection changes bump this version and re-seed affected fields.
pub const PROFILE_VERSION: u16 = 1;

/// A fact family the baseline holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileField {
    /// Who the certificate says the site is, and who issued it.
    Certificate,
    /// The security-relevant response headers of the entry route.
    SecurityHeaders,
    /// The third-party origins the site's pages load from.
    ThirdPartyOrigins,
    /// Mail and certificate-authority posture in DNS.
    DnsPosture,
    /// The routes known to exist on the site.
    RouteSet,
}

impl ProfileField {
    /// Every family, in display order.
    pub const ALL: &'static [ProfileField] = &[
        ProfileField::Certificate,
        ProfileField::SecurityHeaders,
        ProfileField::ThirdPartyOrigins,
        ProfileField::DnsPosture,
        ProfileField::RouteSet,
    ];

    /// The stored key. Storage and the wire share one spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Certificate => "certificate",
            Self::SecurityHeaders => "security_headers",
            Self::ThirdPartyOrigins => "third_party_origins",
            Self::DnsPosture => "dns_posture",
            Self::RouteSet => "route_set",
        }
    }

    /// Parse a stored key, returning `None` for unknown values.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|f| f.as_str() == value)
    }

    /// User-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Certificate => "Certificate identity",
            Self::SecurityHeaders => "Security headers",
            Self::ThirdPartyOrigins => "Third-party origins",
            Self::DnsPosture => "DNS posture",
            Self::RouteSet => "Known routes",
        }
    }

    /// Whether smaller set observations mean reduced coverage rather than removal.
    pub fn growth_only(self) -> bool {
        matches!(self, Self::ThirdPartyOrigins | Self::RouteSet)
    }
}

/// How a good value came to be good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordOrigin {
    /// The first observation of the field.
    Seeded,
    /// The field came back to a clean value on its own.
    Promoted,
    /// A person accepted the changed value.
    Accepted,
    /// The projection changed, so the stored value was not comparable.
    Reseeded,
}

impl RecordOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seeded => "seeded",
            Self::Promoted => "promoted",
            Self::Accepted => "accepted",
            Self::Reseeded => "reseeded",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Seeded, Self::Promoted, Self::Accepted, Self::Reseeded]
            .into_iter()
            .find(|origin| origin.as_str() == value)
    }
}

/// A good value with the provenance that makes it auditable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldRecord {
    pub value: FieldValue,
    pub digest: String,
    pub profile_version: u16,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scan_id: Option<i64>,
    pub origin: RecordOrigin,
}

/// Current deviation from the verified baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftRecord {
    pub value: FieldValue,
    pub digest: String,
    pub first_seen_at: chrono::DateTime<chrono::Utc>,
    pub last_seen_at: chrono::DateTime<chrono::Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scan_id: Option<i64>,
    /// Suppress notifications without changing the baseline.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dismissed: bool,
}

/// One family's state: what good is, and what (if anything) differs now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldState {
    pub good: FieldRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drift: Option<DriftRecord>,
}

/// The site's baseline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedGoodProfile {
    pub revision: u64,
    pub fields: BTreeMap<ProfileField, FieldState>,
}

/// What an observation did to one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldTransition {
    Seeded,
    Reseeded,
    /// Matched good; any drift cleared.
    Unchanged,
    /// A new difference from good.
    DriftOpened,
    /// The same difference, seen again.
    DriftPersisted,
    /// The difference itself changed, so a dismissal no longer applies.
    DriftMoved,
    /// The value came back; good was re-established.
    Recovered,
    /// User accepted the difference as the new baseline.
    Accepted,
    /// User dismissed the difference without changing the baseline.
    Dismissed,
}

/// Partial field observation; omitted families remain unknown, not empty.
#[derive(Debug, Clone, Default)]
pub struct Observation {
    pub values: Vec<FieldValue>,
    pub scan_id: Option<i64>,
}

impl Observation {
    pub fn push(&mut self, value: FieldValue) {
        self.values.push(value);
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Result of applying an observation or user decision.
#[derive(Debug, Clone)]
pub struct ProfileUpdate {
    pub profile: VerifiedGoodProfile,
    pub transitions: Vec<(ProfileField, FieldTransition)>,
    /// Whether state changed enough to consume a revision.
    pub changed: bool,
}

/// Why an acceptance or dismissal was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionError {
    /// The field has no difference to decide about.
    NoDrift,
    /// Revision or digest no longer matches the value being decided.
    StaleRevision {
        current_revision: u64,
        current_digest: Option<String>,
    },
}

impl DecisionError {
    /// The closed-vocabulary code, shared with the protocol's error set.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoDrift => "no_drift",
            Self::StaleRevision { .. } => "stale_revision",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NoDrift => {
                "This baseline has nothing to accept: the site matches what was recorded as good."
                    .into()
            }
            Self::StaleRevision { .. } => {
                "The site changed again while this was open. Review the current value before accepting it as the baseline.".into()
            }
        }
    }
}

impl VerifiedGoodProfile {
    /// Apply an observation. Pure: the caller stores what comes back.
    pub fn observe(
        &self,
        observation: &Observation,
        observed_at: chrono::DateTime<chrono::Utc>,
    ) -> ProfileUpdate {
        let mut profile = self.clone();
        let mut transitions = Vec::new();
        for value in &observation.values {
            let field = value.field();
            let transition = match profile.fields.get(&field) {
                None => {
                    profile.fields.insert(
                        field,
                        FieldState {
                            good: seed_record(
                                value.clone(),
                                observed_at,
                                observation.scan_id,
                                RecordOrigin::Seeded,
                            ),
                            drift: None,
                        },
                    );
                    FieldTransition::Seeded
                }
                Some(state) if state.good.profile_version != PROFILE_VERSION => {
                    profile.fields.insert(
                        field,
                        FieldState {
                            good: seed_record(
                                value.clone(),
                                observed_at,
                                observation.scan_id,
                                RecordOrigin::Reseeded,
                            ),
                            drift: None,
                        },
                    );
                    FieldTransition::Reseeded
                }
                Some(state) => {
                    let (next, transition) =
                        observe_field(state, value, observed_at, observation.scan_id);
                    if let Some(next) = next {
                        profile.fields.insert(field, next);
                    }
                    transition
                }
            };
            transitions.push((field, transition));
        }
        let changed = profile.fields != self.fields;
        if changed {
            profile.revision = self.revision.saturating_add(1);
        }
        ProfileUpdate {
            profile,
            transitions,
            changed,
        }
    }

    /// Accept the current difference after checking its revision and digest.
    pub fn accept(
        &self,
        field: ProfileField,
        based_on_revision: u64,
        expected_digest: &str,
        accepted_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<ProfileUpdate, DecisionError> {
        let state = self.guard(field, based_on_revision, expected_digest)?;
        let drift = state.drift.as_ref().expect("guarded drift");
        let mut profile = self.clone();
        profile.fields.insert(
            field,
            FieldState {
                good: FieldRecord {
                    value: drift.value.clone(),
                    digest: drift.digest.clone(),
                    profile_version: PROFILE_VERSION,
                    recorded_at: accepted_at,
                    source_scan_id: drift.source_scan_id,
                    origin: RecordOrigin::Accepted,
                },
                drift: None,
            },
        );
        profile.revision = self.revision.saturating_add(1);
        Ok(ProfileUpdate {
            profile,
            transitions: vec![(field, FieldTransition::Accepted)],
            changed: true,
        })
    }

    /// Silence the current difference. Good does not move: this is the other
    /// half of the distinction the lifecycle exists to keep.
    pub fn dismiss(
        &self,
        field: ProfileField,
        based_on_revision: u64,
        expected_digest: &str,
    ) -> Result<ProfileUpdate, DecisionError> {
        let state = self.guard(field, based_on_revision, expected_digest)?;
        let mut state = state.clone();
        if let Some(drift) = state.drift.as_mut() {
            drift.dismissed = true;
        }
        let mut profile = self.clone();
        profile.fields.insert(field, state);
        profile.revision = self.revision.saturating_add(1);
        Ok(ProfileUpdate {
            profile,
            transitions: vec![(field, FieldTransition::Dismissed)],
            changed: true,
        })
    }

    fn guard(
        &self,
        field: ProfileField,
        based_on_revision: u64,
        expected_digest: &str,
    ) -> Result<&FieldState, DecisionError> {
        let Some(state) = self.fields.get(&field) else {
            return Err(DecisionError::NoDrift);
        };
        let Some(drift) = state.drift.as_ref() else {
            return Err(DecisionError::NoDrift);
        };
        if based_on_revision != self.revision || expected_digest != drift.digest {
            return Err(DecisionError::StaleRevision {
                current_revision: self.revision,
                current_digest: Some(drift.digest.clone()),
            });
        }
        Ok(state)
    }

    /// The families showing a difference a person has not silenced.
    pub fn open_drift(&self) -> Vec<(ProfileField, &DriftRecord)> {
        self.fields
            .iter()
            .filter_map(|(field, state)| {
                state
                    .drift
                    .as_ref()
                    .filter(|drift| !drift.dismissed)
                    .map(|drift| (*field, drift))
            })
            .collect()
    }
}

fn seed_record(
    value: FieldValue,
    recorded_at: chrono::DateTime<chrono::Utc>,
    source_scan_id: Option<i64>,
    origin: RecordOrigin,
) -> FieldRecord {
    FieldRecord {
        digest: value.digest(),
        value,
        profile_version: PROFILE_VERSION,
        recorded_at,
        source_scan_id,
        origin,
    }
}

/// One field's transition. Returns `None` for the state when nothing changed,
/// so an unchanged observation cannot burn a revision.
fn observe_field(
    state: &FieldState,
    observed: &FieldValue,
    observed_at: chrono::DateTime<chrono::Utc>,
    scan_id: Option<i64>,
) -> (Option<FieldState>, FieldTransition) {
    match state.good.value.compare(observed) {
        Comparison::Match => {
            if state.drift.is_none() {
                return (None, FieldTransition::Unchanged);
            }
            if observed.field().growth_only() {
                return (None, FieldTransition::Unchanged);
            }
            // The value came back on its own: good is re-established with the
            // provenance of the run that proved it, and the difference is gone.
            (
                Some(FieldState {
                    good: FieldRecord {
                        recorded_at: observed_at,
                        source_scan_id: scan_id,
                        origin: RecordOrigin::Promoted,
                        ..state.good.clone()
                    },
                    drift: None,
                }),
                FieldTransition::Recovered,
            )
        }
        Comparison::Thinner => (None, FieldTransition::Unchanged),
        Comparison::Changed(value) => {
            let digest = value.digest();
            match state.drift.as_ref() {
                Some(previous) if previous.digest == digest => {
                    let mut drift = previous.clone();
                    drift.last_seen_at = observed_at;
                    drift.source_scan_id = scan_id;
                    (
                        Some(FieldState {
                            good: state.good.clone(),
                            drift: Some(drift),
                        }),
                        FieldTransition::DriftPersisted,
                    )
                }
                Some(_) => (
                    Some(FieldState {
                        good: state.good.clone(),
                        // A different difference: the dismissal covered the
                        // value that was dismissed, not this one.
                        drift: Some(open_drift(value, digest, observed_at, scan_id)),
                    }),
                    FieldTransition::DriftMoved,
                ),
                None => (
                    Some(FieldState {
                        good: state.good.clone(),
                        drift: Some(open_drift(value, digest, observed_at, scan_id)),
                    }),
                    FieldTransition::DriftOpened,
                ),
            }
        }
    }
}

fn open_drift(
    value: FieldValue,
    digest: String,
    observed_at: chrono::DateTime<chrono::Utc>,
    scan_id: Option<i64>,
) -> DriftRecord {
    DriftRecord {
        value,
        digest,
        first_seen_at: observed_at,
        last_seen_at: observed_at,
        source_scan_id: scan_id,
        dismissed: false,
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
