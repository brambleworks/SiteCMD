//! Source-specific connected-service snapshots with coverage-bound resolution.

use serde::{Deserialize, Serialize};

use crate::coverage::{CoverageExceptionReason, ScanCoverageKind};
use crate::route::CanonicalRoute;
use crate::vocab::{IssueConfidence, Severity};

/// Engine versions pinned by one Web snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebVersions {
    pub engine_release: String,
    pub fingerprint_schema: u16,
    pub canonicalizer: u16,
    pub crawl_profile: u16,
}

/// Code-snapshot versions, including the fingerprint key but no crawl profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeVersions {
    pub engine_release: String,
    pub fingerprint_schema: u16,
    pub fingerprint_key_version: u16,
    pub canonicalizer: u16,
}

/// Browser identity reported by the producer.
/// The optional build lets the service resolve compatibility without inventing
/// a version when the runtime cannot observe one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub engine: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
}

/// Producer-declared runtime facts used for comparability. Instance identity
/// and locality are server-derived and cannot be claimed by the client.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireExecutionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<BrowserProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axe_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_adapter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_adapter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_authority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_profile: Option<String>,
    /// Which layers the observation ran. A layer that did not run cannot be
    /// read as absence of what it would have found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers_run: Vec<String>,
}

impl WireExecutionProfile {
    /// Projects engine facts onto the wire, excluding `browser_epoch` because
    /// only the server certification registry can establish it.
    pub fn from_execution(profile: &crate::release::ExecutionProfile) -> Self {
        let browser = profile
            .browser_engine
            .as_ref()
            .map(|engine| BrowserProfile {
                engine: engine.clone(),
                build: profile.browser_build.clone(),
            });
        Self {
            browser,
            axe_version: profile.axe_version.clone(),
            resolver: profile.resolver.clone(),
            transport_adapter: profile.transport.clone(),
            tls_adapter: profile.tls_client.clone(),
            trust_authority: profile.trust_authority.clone(),
            scan_profile: profile.scan_profile.clone(),
            layers_run: profile.layers_run.clone(),
        }
    }
}

/// One uncovered route-check pair with its explicit coverage reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireCoverageException {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    pub checks_not_run: Vec<String>,
    pub reason: CoverageExceptionReason,
}

/// Pair-level snapshot authority. `complete` claims the route-check product;
/// `exceptions` removes individual pairs from that claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireCoverage {
    pub kind: ScanCoverageKind,
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exceptions: Vec<WireCoverageException>,
}

/// Web finding identity plus the severity and confidence inputs required by
/// the shared scorer for weighting, caps, and deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebOccurrence {
    pub check: String,
    /// Absent for a site-scoped cross-page finding. Assigning the environment
    /// root would falsely make one route capable of verifying a set-level
    /// finding, so route absence is part of the identity contract.
    #[serde(default, flatten, skip_serializing_if = "Option::is_none")]
    pub route: Option<CanonicalRoute>,
    /// The canonical authored route whose execution reached `route`. It can
    /// differ after a redirect and governs scope and absence coverage, while
    /// `route` remains the final resource identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_route: Option<String>,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<IssueConfidence>,
}

/// Checkout provenance claims available to an unattested desktop.
/// Exact and unattested verdicts remain server-assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopProvenanceKind {
    /// A known-ancestor commit with the relevant files unchanged since. Only
    /// the side holding git can compute this, which is why no server-side
    /// corroboration can mint it.
    Compatible,
    /// Local evidence predates the deployment.
    Stale,
    /// No trustworthy code-side baseline exists.
    Unknown,
}

/// The commit a code occurrence was observed against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeProvenance {
    #[serde(default)]
    pub commit_sha: Option<String>,
    pub kind: DesktopProvenanceKind,
}

/// A code pair's identity: the check, and the keyed hash of where it fired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodePairIdentity {
    pub check: String,
    pub location_hash: String,
}

/// Whether a code snapshot may resolve absence, and for which pairs.
/// Snapshot-level authority covers clean scans; `unvouched` excludes pairs in
/// paths whose compatibility cannot be established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeBasis {
    #[serde(default)]
    pub commit_sha: Option<String>,
    pub kind: CodeBasisKind,
    /// The pairs this basis does NOT cover.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unvouched: Vec<CodePairIdentity>,
}

/// The four bases, two of which may resolve absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeBasisKind {
    /// Clean working tree at the deployed SHA, self-reported by the desktop.
    ExactCheckout,
    /// A known ancestor with the relevant files unchanged since.
    /// Machine-independent, so one installation's compatible snapshot soundly
    /// clears what another established.
    Compatible,
    /// Older than the current deployment. Informs presence, resolves nothing.
    Stale,
    /// No usable relationship to the deployed commit.
    Unknown,
}

impl CodeBasisKind {
    /// Whether a basis of this kind may resolve absence for pairs it vouches
    /// for. Stale and unknown bases inform presence and never clear.
    pub fn may_resolve_absence(self) -> bool {
        matches!(self, Self::ExactCheckout | Self::Compatible)
    }
}

/// Code finding identity, multiplicity, provenance, and score inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeOccurrence {
    pub check: String,
    pub location_hash: String,
    pub instance_count: u32,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<IssueConfidence>,
    pub provenance: CodeProvenance,
}

/// Timing sample excluded from lifecycle state because it varies by vantage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementSample {
    pub check: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    pub value: f64,
    /// The unit, which the capability manifest is the authority on. A sample
    /// whose unit disagrees with its check's manifest entry is rejected rather
    /// than charted against values it cannot be compared to.
    pub unit: String,
}

/// Framework identity without dependency or lockfile contents.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackFacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework_version: Option<String>,
}

/// A completed web scan, as the service reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebSnapshot {
    pub observed_at: i64,
    /// Latest site event sequence known to the producer when scanning.
    pub based_on_event_sequence: i64,
    pub versions: WebVersions,
    pub manifest_digest: String,
    /// Injected evaluation time used for time-sensitive cause classification.
    pub evaluation_time: i64,
    pub execution_profile: WireExecutionProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_facts: Option<StackFacts>,
    pub coverage: WireCoverage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub occurrences: Vec<WebOccurrence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measurement_samples: Vec<MeasurementSample>,
}

/// A completed code scan, as the service reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeSnapshot {
    pub observed_at: i64,
    pub based_on_event_sequence: i64,
    pub versions: CodeVersions,
    pub manifest_digest: String,
    pub evaluation_time: i64,
    pub execution_profile: WireExecutionProfile,
    /// The commitment of the key these fingerprints were computed under, so a
    /// producer hashing under the wrong key for a claimed version fails
    /// visibly instead of corrupting identity matching in silence.
    pub key_commitment: String,
    pub code_basis: CodeBasis,
    pub coverage: WireCoverage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub occurrences: Vec<CodeOccurrence>,
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
