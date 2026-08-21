//! Bootstrap projection from findings, lifecycle overrides, and retained occurrences.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension};

use crate::checks::{IssueConfidence, Severity};
use crate::core::normalized_scan::{ScanEvidenceSource, ScanFindingLocationKind};
use crate::core::types_work_items::{IssueStatus, VerifiedBy};

use super::helpers::{lifecycle_env_url, parse_optional_enum_required, parse_required_enum};
use super::{Database, DbError};

/// Read-only bootstrap lifecycle, including stored `Regressed` state. The
/// producer write vocabulary intentionally cannot declare regressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapState {
    Active,
    Snoozed { until: i64 },
    Ignored,
    Blocked { reason: Option<String> },
    Verified { by: VerifiedBy },
    Regressed,
}

impl BootstrapState {
    /// Whether verification requires last-known occurrence identities.
    pub fn awaits_verification(&self) -> bool {
        matches!(self, BootstrapState::Verified { .. })
    }
}

/// Local occurrence identity before wire canonicalization.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OccurrenceLocation {
    /// Observed on one page, stored exactly as the scan observed it.
    Page { url: String },
    /// Repository-relative code identity without unstable line numbers.
    File { rule: String, path: String },
    /// Cross-page or project-wide finding without a narrower location.
    Whole,
}

/// An occurrence stripped to what makes it that occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccurrenceIdentity {
    pub source: ScanEvidenceSource,
    pub check_id: String,
    pub location: OccurrenceLocation,
    /// Number of findings collapsed into this identity.
    pub instance_count: u32,
}

/// Bootstrap occurrence with authored routes kept outside its final identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastKnownOccurrenceRecord {
    pub identity: OccurrenceIdentity,
    pub authored_page_urls: Vec<String>,
}

/// An occurrence as one scan saw it: its identity plus the two observation
/// facts the shared scorer needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedOccurrence {
    pub identity: OccurrenceIdentity,
    pub severity: Severity,
    pub confidence: IssueConfidence,
}

/// One group as bootstrap will declare it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapGroup {
    pub check_id: String,
    pub state: BootstrapState,
    pub state_changed_at: i64,
    /// Which scanners produced this group. Empty when local evidence no longer
    /// says: a terminal group whose scans have aged out, or a group whose only
    /// evidence came from an integration signal rather than a scan.
    pub sources: Vec<ScanEvidenceSource>,
    /// Where the group was when it was last seen. Populated only for groups
    /// awaiting verification, and empty when local history no longer reaches
    /// back that far.
    pub last_known_occurrences: Vec<LastKnownOccurrenceRecord>,
}

/// The latest complete scan of one source, and what it observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEvidence {
    pub source: ScanEvidenceSource,
    pub execution_id: i64,
    /// Complete runs composing this source snapshot.
    pub run_ids: Vec<i64>,
    pub observed_at: i64,
    /// The event watermark this evidence was gathered against, captured on the
    /// execution when the scan started looking.
    pub based_on_event_sequence: i64,
    pub occurrences: Vec<ObservedOccurrence>,
}

/// Everything a bootstrap submission is derived from, taken from one read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapSet {
    pub groups: Vec<BootstrapGroup>,
    pub evidence: Vec<SourceEvidence>,
}

/// Map local projection labels back to their scan evidence source.
fn evidence_source_of(projection_source: &str) -> Option<ScanEvidenceSource> {
    match projection_source {
        "web_scan" | "site_scan" => Some(ScanEvidenceSource::WebScan),
        "code_scan" => Some(ScanEvidenceSource::CodeScan),
        _ => None,
    }
}

/// Confidence as a strength, so collapsing instances can keep the strongest.
fn confidence_rank(confidence: IssueConfidence) -> u8 {
    match confidence {
        IssueConfidence::Confirmed => 2,
        IssueConfidence::High => 1,
        IssueConfidence::NeedsReview => 0,
    }
}

fn location_of(
    kind: ScanFindingLocationKind,
    page_url: Option<String>,
    relative_path: Option<String>,
    producer_check_id: String,
) -> OccurrenceLocation {
    match kind {
        ScanFindingLocationKind::Page => page_url
            .filter(|url| !url.is_empty())
            .map_or(OccurrenceLocation::Whole, |url| OccurrenceLocation::Page {
                url,
            }),
        ScanFindingLocationKind::File => relative_path.filter(|path| !path.is_empty()).map_or(
            OccurrenceLocation::Whole,
            |path| OccurrenceLocation::File {
                rule: producer_check_id,
                path,
            },
        ),
        ScanFindingLocationKind::Project
        | ScanFindingLocationKind::Site
        | ScanFindingLocationKind::None => OccurrenceLocation::Whole,
    }
}

/// One finding as read from `scan_findings`, before collapse.
struct FindingRow {
    check_id: String,
    location: OccurrenceLocation,
    severity: Severity,
    confidence: IssueConfidence,
    authored_page_url: Option<String>,
}

type OccurrenceKey = (ScanEvidenceSource, String, OccurrenceLocation);

struct Collapsed {
    instance_count: u32,
    severity: Severity,
    confidence: IssueConfidence,
    authored_page_urls: BTreeSet<String>,
}

/// Collapse duplicate rows while retaining strongest severity and confidence.
fn collapse(
    source: ScanEvidenceSource,
    rows: Vec<FindingRow>,
) -> BTreeMap<OccurrenceKey, Collapsed> {
    let mut folded: BTreeMap<OccurrenceKey, Collapsed> = BTreeMap::new();
    for row in rows {
        let entry = folded
            .entry((source, row.check_id, row.location))
            .or_insert(Collapsed {
                instance_count: 0,
                severity: row.severity,
                confidence: row.confidence,
                authored_page_urls: BTreeSet::new(),
            });
        entry.instance_count = entry.instance_count.saturating_add(1);
        if let Some(authored_page_url) = row.authored_page_url {
            entry.authored_page_urls.insert(authored_page_url);
        }
        if row.severity.impact_rank() > entry.severity.impact_rank() {
            entry.severity = row.severity;
        }
        if confidence_rank(row.confidence) > confidence_rank(entry.confidence) {
            entry.confidence = row.confidence;
        }
    }
    folded
}

fn into_last_known(folded: BTreeMap<OccurrenceKey, Collapsed>) -> Vec<LastKnownOccurrenceRecord> {
    folded
        .into_iter()
        .map(
            |((source, check_id, location), collapsed)| LastKnownOccurrenceRecord {
                identity: OccurrenceIdentity {
                    source,
                    check_id,
                    location,
                    instance_count: collapsed.instance_count,
                },
                authored_page_urls: collapsed.authored_page_urls.into_iter().collect(),
            },
        )
        .collect()
}

fn into_observations(folded: BTreeMap<OccurrenceKey, Collapsed>) -> Vec<ObservedOccurrence> {
    folded
        .into_iter()
        .map(
            |((source, check_id, location), collapsed)| ObservedOccurrence {
                severity: collapsed.severity,
                confidence: collapsed.confidence,
                identity: OccurrenceIdentity {
                    source,
                    check_id,
                    location,
                    instance_count: collapsed.instance_count,
                },
            },
        )
        .collect()
}

/// Decode lifecycle state, rejecting snoozes without a deadline.
fn state_from_row(
    check_id: &str,
    status: IssueStatus,
    snooze_until: Option<i64>,
    block_reason: Option<String>,
    verified_by: Option<VerifiedBy>,
) -> Result<BootstrapState, DbError> {
    Ok(match status {
        IssueStatus::New => BootstrapState::Active,
        IssueStatus::Snoozed => BootstrapState::Snoozed {
            until: snooze_until.ok_or_else(|| {
                DbError::Other(format!(
                    "snoozed group {check_id} has no deadline to declare"
                ))
            })?,
        },
        IssueStatus::Ignored => BootstrapState::Ignored,
        IssueStatus::Blocked => BootstrapState::Blocked {
            reason: block_reason,
        },
        IssueStatus::Verified => BootstrapState::Verified {
            by: verified_by.ok_or_else(|| {
                DbError::Other(format!("verified group {check_id} has no prover to name"))
            })?,
        },
        IssueStatus::Regressed => BootstrapState::Regressed,
    })
}

/// A group under construction: what the projection says, then what the
/// override says.
#[derive(Default)]
struct Draft {
    present: bool,
    first_seen_at: Option<i64>,
    sources: BTreeSet<ScanEvidenceSource>,
    state: Option<BootstrapState>,
    state_changed_at: Option<i64>,
}

fn read_projection(
    conn: &Connection,
    project_id: i64,
    env_key: &str,
    drafts: &mut BTreeMap<String, Draft>,
) -> Result<(), DbError> {
    let mut statement = conn.prepare(
        "SELECT check_id, source,
                MIN(CASE WHEN resolved_at IS NULL THEN first_seen_at END),
                SUM(CASE WHEN resolved_at IS NULL THEN 1 ELSE 0 END)
         FROM work_items
         WHERE project_id = ?1 AND env_url = ?2
         GROUP BY check_id, source",
    )?;
    let rows = statement
        .query_map(params![project_id, env_key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (check_id, projection_source, first_seen_at, open_count) in rows {
        let draft = drafts.entry(check_id).or_default();
        if let Some(source) = evidence_source_of(&projection_source) {
            draft.sources.insert(source);
        }
        if open_count > 0 {
            draft.present = true;
            draft.first_seen_at = match (draft.first_seen_at, first_seen_at) {
                (Some(current), Some(candidate)) => Some(current.min(candidate)),
                (current, candidate) => current.or(candidate),
            };
        }
    }
    Ok(())
}

fn read_overrides(
    conn: &Connection,
    project_id: i64,
    env_key: &str,
    drafts: &mut BTreeMap<String, Draft>,
) -> Result<(), DbError> {
    let mut statement = conn.prepare(
        "SELECT check_id, status, snooze_until, block_reason, verified_by,
                last_status_changed_at
         FROM project_issue_states
         WHERE project_id = ?1 AND env_url = ?2",
    )?;
    let rows = statement
        .query_map(params![project_id, env_key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                parse_required_enum(1, "project_issue_states.status", &row.get::<_, String>(1)?)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                parse_optional_enum_required::<VerifiedBy>(
                    4,
                    "project_issue_states.verified_by",
                    row.get::<_, Option<String>>(4)?,
                )?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (check_id, status, snooze_until, block_reason, verified_by, changed_at) in rows {
        let state = state_from_row(&check_id, status, snooze_until, block_reason, verified_by)?;
        let draft = drafts.entry(check_id).or_default();
        draft.state = Some(state);
        draft.state_changed_at = Some(changed_at);
    }
    Ok(())
}

/// Occurrence identities from the newest complete execution per source. Using
/// an execution preserves every route from multi-page scans.
fn last_known_occurrences(
    conn: &Connection,
    project_id: i64,
    env_key: &str,
    source: ScanEvidenceSource,
) -> Result<BTreeMap<String, Vec<FindingRow>>, DbError> {
    let mut statement = conn.prepare(
        "WITH present AS (
             SELECT DISTINCT finding.canonical_check_id AS check_id,
                             run.execution_id AS execution_id,
                             execution.started_at AS started_at
             FROM scan_findings finding
             JOIN scan_runs run ON run.id = finding.run_id
             JOIN scan_executions execution ON execution.id = run.execution_id
             WHERE run.project_id = ?1
               AND run.environment_scope_key = ?2
               AND run.source = ?3
               AND run.status = 'complete'
               AND finding.verdict IN ('fail', 'warn')
         ),
         newest AS (
             SELECT check_id, execution_id,
                    ROW_NUMBER() OVER (
                        PARTITION BY check_id
                        ORDER BY started_at DESC, execution_id DESC
                    ) AS recency
             FROM present
         )
         SELECT newest.check_id, finding.producer_check_id, finding.location_kind,
                finding.page_url, finding.relative_path, finding.severity,
                finding.confidence, run.page_url
         FROM newest
         JOIN scan_runs run ON run.execution_id = newest.execution_id
         JOIN scan_findings finding ON finding.run_id = run.id
         WHERE newest.recency = 1
           AND run.source = ?3
           AND run.status = 'complete'
           AND finding.canonical_check_id = newest.check_id
           AND finding.verdict IN ('fail', 'warn')
         ORDER BY newest.check_id, finding.run_id, finding.ordinal",
    )?;
    let rows = statement
        .query_map(params![project_id, env_key, source.as_str()], |row| {
            let mut finding = read_finding(row)?;
            finding.authored_page_url = row.get(7)?;
            Ok(finding)
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut by_check: BTreeMap<String, Vec<FindingRow>> = BTreeMap::new();
    for row in rows {
        by_check.entry(row.check_id.clone()).or_default().push(row);
    }
    Ok(by_check)
}

fn read_finding(row: &rusqlite::Row<'_>) -> rusqlite::Result<FindingRow> {
    let producer_check_id: String = row.get(1)?;
    let kind: ScanFindingLocationKind =
        parse_required_enum(2, "scan_findings.location_kind", &row.get::<_, String>(2)?)?;
    Ok(FindingRow {
        check_id: row.get(0)?,
        location: location_of(kind, row.get(3)?, row.get(4)?, producer_check_id),
        severity: parse_required_enum(5, "scan_findings.severity", &row.get::<_, String>(5)?)?,
        confidence: parse_required_enum(6, "scan_findings.confidence", &row.get::<_, String>(6)?)?,
        authored_page_url: None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidenceSelection {
    Latest,
    PreferNonBounded,
}

fn latest_evidence(
    conn: &Connection,
    project_id: i64,
    env_key: &str,
    source: ScanEvidenceSource,
    selection: EvidenceSelection,
) -> Result<Option<SourceEvidence>, DbError> {
    let prefer_non_bounded = i64::from(selection == EvidenceSelection::PreferNonBounded);
    let Some((execution_id, based_on_event_sequence, observed_at)) = conn
        .query_row(
            "SELECT execution.id,
                    execution.based_on_event_sequence,
                    COALESCE(MAX(run.completed_at), MAX(run.started_at))
             FROM scan_executions execution
             JOIN scan_runs run ON run.execution_id = execution.id
             WHERE execution.project_id = :project_id
               AND execution.environment_scope_key = :env_key
               AND run.source = :source
               AND run.status = 'complete'
             GROUP BY execution.id
             ORDER BY CASE
                          WHEN :prefer_non_bounded = 1
                               AND execution.admission_class = 'bounded_verification'
                              THEN 1
                          ELSE 0
                      END,
                      execution.started_at DESC, execution.id DESC
             LIMIT 1",
            rusqlite::named_params! {
                ":project_id": project_id,
                ":env_key": env_key,
                ":source": source.as_str(),
                ":prefer_non_bounded": prefer_non_bounded,
            },
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };

    let mut run_statement = conn.prepare(
        "SELECT id FROM scan_runs
         WHERE execution_id = ?1 AND source = ?2 AND status = 'complete'
         ORDER BY id",
    )?;
    let run_ids = run_statement
        .query_map(params![execution_id, source.as_str()], |row| {
            row.get::<_, i64>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut finding_statement = conn.prepare(
        "SELECT finding.canonical_check_id, finding.producer_check_id,
                finding.location_kind, finding.page_url, finding.relative_path,
                finding.severity, finding.confidence
         FROM scan_findings finding
         JOIN scan_runs run ON run.id = finding.run_id
         WHERE run.execution_id = ?1
           AND run.source = ?2
           AND run.status = 'complete'
           AND finding.verdict IN ('fail', 'warn')
         ORDER BY finding.run_id, finding.ordinal",
    )?;
    let findings = finding_statement
        .query_map(params![execution_id, source.as_str()], read_finding)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(SourceEvidence {
        source,
        execution_id,
        run_ids,
        observed_at,
        based_on_event_sequence,
        occurrences: into_observations(collapse(source, findings)),
    }))
}

impl Database {
    /// Derive a bootstrap group set and evidence in one transaction.
    /// This does not require a connection so the inspector can preview it first.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn derive_bootstrap_set(
        &self,
        project_id: i64,
        environment_scope_key: &str,
    ) -> Result<BootstrapSet, DbError> {
        let env_key = lifecycle_env_url(environment_scope_key);
        if env_key.is_empty() {
            return Err("an environment is required to derive a bootstrap set".into());
        }
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;

            let mut drafts: BTreeMap<String, Draft> = BTreeMap::new();
            read_projection(&tx, project_id, &env_key, &mut drafts)?;
            read_overrides(&tx, project_id, &env_key, &mut drafts)?;

            let awaiting: BTreeSet<String> = drafts
                .iter()
                .filter(|(_, draft)| {
                    draft
                        .state
                        .as_ref()
                        .is_some_and(BootstrapState::awaits_verification)
                })
                .map(|(check_id, _)| check_id.clone())
                .collect();
            let mut tombstones: Vec<(ScanEvidenceSource, BTreeMap<String, Vec<FindingRow>>)> =
                Vec::new();
            if !awaiting.is_empty() {
                for source in [ScanEvidenceSource::WebScan, ScanEvidenceSource::CodeScan] {
                    tombstones.push((
                        source,
                        last_known_occurrences(&tx, project_id, &env_key, source)?,
                    ));
                }
            }

            let mut evidence = Vec::new();
            for source in [ScanEvidenceSource::WebScan, ScanEvidenceSource::CodeScan] {
                if let Some(found) = latest_evidence(
                    &tx,
                    project_id,
                    &env_key,
                    source,
                    EvidenceSelection::PreferNonBounded,
                )? {
                    evidence.push(found);
                }
            }
            tx.commit()?;

            let mut groups = Vec::new();
            for (check_id, mut draft) in drafts {
                // A group with only resolved rows and no override was fixed by
                // a scan and never given a state. The service never knew it,
                // and reviving it here would report a fix as an open issue.
                if !draft.present && draft.state.is_none() {
                    continue;
                }
                let mut last_known = Vec::new();
                if awaiting.contains(&check_id) {
                    for (source, by_check) in &tombstones {
                        let Some(rows) = by_check.get(&check_id) else {
                            continue;
                        };
                        // A group whose projection rows aged out still names
                        // its source here: the scan that last saw it knows
                        // which scanner it was.
                        draft.sources.insert(*source);
                        let cloned = rows
                            .iter()
                            .map(|row| FindingRow {
                                check_id: row.check_id.clone(),
                                location: row.location.clone(),
                                severity: row.severity,
                                confidence: row.confidence,
                                authored_page_url: row.authored_page_url.clone(),
                            })
                            .collect();
                        last_known.extend(into_last_known(collapse(*source, cloned)));
                    }
                }
                let state = draft.state.unwrap_or(BootstrapState::Active);
                // An untouched group entered its state when it was first seen;
                // there is no status change to date it by, because nobody
                // changed anything.
                let state_changed_at = draft
                    .state_changed_at
                    .or(draft.first_seen_at)
                    .unwrap_or_default();
                groups.push(BootstrapGroup {
                    check_id,
                    state,
                    state_changed_at,
                    sources: draft.sources.into_iter().collect(),
                    last_known_occurrences: last_known,
                });
            }

            Ok(BootstrapSet { groups, evidence })
        })?
    }

    /// Read the newest post-bootstrap evidence from each source in one
    /// snapshot. No groups are returned because a non-bootstrap submission
    /// cannot create them; bounded verification evidence is eligible here.
    pub(crate) fn latest_submission_evidence(
        &self,
        project_id: i64,
        environment_scope_key: &str,
    ) -> Result<Vec<SourceEvidence>, DbError> {
        let env_key = lifecycle_env_url(environment_scope_key);
        if env_key.is_empty() {
            return Err("an environment is required to read scan evidence".into());
        }
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let mut evidence = Vec::new();
            for source in [ScanEvidenceSource::WebScan, ScanEvidenceSource::CodeScan] {
                if let Some(found) =
                    latest_evidence(&tx, project_id, &env_key, source, EvidenceSelection::Latest)?
                {
                    evidence.push(found);
                }
            }
            tx.commit()?;
            Ok(evidence)
        })?
    }

    /// The latest complete scan of one source and the occurrences it observed.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn latest_source_evidence(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        source: ScanEvidenceSource,
    ) -> Result<Option<SourceEvidence>, DbError> {
        let env_key = lifecycle_env_url(environment_scope_key);
        if env_key.is_empty() {
            return Err("an environment is required to read scan evidence".into());
        }
        self.execute(move |conn| {
            latest_evidence(
                conn,
                project_id,
                &env_key,
                source,
                EvidenceSelection::Latest,
            )
        })?
    }
}

#[cfg(test)]
#[path = "connected_bootstrap_tests.rs"]
mod tests;
