//! Release provenance and per-check observation comparability.
//!
//! Stamps record build and runtime facts; inventories preserve the checks and
//! contracts each release could produce.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manifest::{CapabilityManifest, CompareDimension};

#[cfg(test)]
#[path = "release_tests.rs"]
mod tests;

/// Canonical-form version for identities and digests. Increment when
/// canonicalization changes.
pub const CANONICALIZER_VERSION: u16 = 1;

/// The fingerprint schema version the protocol envelope carries.
pub const FINGERPRINT_SCHEMA: u16 = 1;

/// Version for page discovery, ordering, and bounds, independent of check
/// semantics in the capability manifest.
pub const CRAWL_PROFILE: u16 = 1;

/// Runtime facts that may affect a verdict beyond the check implementation.
/// Missing fields mean the producer does not know the value; cross-vantage
/// comparisons must declare every dimension they rely on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionProfile {
    /// The browser that produced browser-lane results, or `None` when the run
    /// had no browser at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_engine: Option<String>,
    /// Exact browser build used by the run. Reported for server-side mapping
    /// to a certified compatibility epoch, but not compared directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_build: Option<String>,
    /// Corpus-certified browser compatibility epoch. Desktop producers leave
    /// this unset because only the hosted runner owns the certification data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_epoch: Option<String>,
    /// The pinned axe-core version, stated only when axe actually ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axe_version: Option<String>,
    /// Which resolver answered DNS questions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver: Option<String>,
    /// The HTTP client profile that fetched pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    /// The TLS client profile that performed handshakes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_client: Option<String>,
    /// The trust anchors chain validation ran against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_authority: Option<String>,
    /// The scan profile the run executed under (its focus or mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_profile: Option<String>,
    /// Which layers the run executed. A layer that did not run cannot be read
    /// as absence of what it would have found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers_run: Vec<String>,
}

impl ExecutionProfile {
    /// The stated value for one comparison dimension, or `None` when this
    /// producer cannot state it.
    pub fn dimension(&self, dimension: CompareDimension) -> Option<&str> {
        let value = match dimension {
            CompareDimension::BrowserEngine => &self.browser_engine,
            CompareDimension::BrowserEpoch => &self.browser_epoch,
            CompareDimension::AxeVersion => &self.axe_version,
            CompareDimension::TransportProfile => &self.transport,
            CompareDimension::TrustAuthority => &self.trust_authority,
            CompareDimension::TlsClientProfile => &self.tls_client,
        };
        value.as_deref()
    }
}

/// What a build states about itself, stamped onto every observation it
/// produces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseStamp {
    /// The release the producing runtime ships as.
    pub engine_release: String,
    /// The capability manifest that governed the run.
    pub manifest_digest: String,
    pub canonicalizer: u16,
    pub crawl_profile: u16,
    pub execution: ExecutionProfile,
}

impl ReleaseStamp {
    /// Stamp for the CURRENT build. The manifest digest is read from the
    /// manifest this binary carries rather than passed in, so a stamp can
    /// never name a document the build does not actually hold.
    pub fn current(engine_release: impl Into<String>, execution: ExecutionProfile) -> Self {
        Self {
            engine_release: engine_release.into(),
            manifest_digest: crate::manifest::capability_manifest().manifest_digest,
            canonicalizer: CANONICALIZER_VERSION,
            crawl_profile: CRAWL_PROFILE,
            execution,
        }
    }
}

/// One check as a past build knew it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    /// Semantic compatibility hash, or `None` when only existence is attested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    /// The dimensions that must additionally match before two readings of this
    /// check compare.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compare_on: Vec<CompareDimension>,
    /// Whether the id is a family PREFIX (`accessibility.axe.`) rather than a
    /// literal check id.
    #[serde(default, skip_serializing_if = "is_false")]
    pub family: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Persistent inventory of checks available in one build.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckInventory {
    entries: BTreeMap<String, InventoryEntry>,
}

impl CheckInventory {
    /// The web checks a capability manifest governs, with their contracts.
    pub fn from_manifest(manifest: &CapabilityManifest) -> Self {
        let entries = manifest
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.check.clone(),
                    InventoryEntry {
                        contract: Some(entry.contract.clone()),
                        compare_on: entry.compare_on.clone(),
                        family: entry.family,
                    },
                )
            })
            .collect();
        Self { entries }
    }

    /// Add enumerable checks without semantic contract hashes.
    pub fn with_unversioned<I, S>(mut self, check_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for check_id in check_ids {
            self.entries
                .entry(check_id.into())
                .or_insert_with(|| InventoryEntry {
                    contract: None,
                    compare_on: Vec::new(),
                    family: false,
                });
        }
        self
    }

    /// Build an inventory from rows read back out of storage.
    pub fn from_entries<I, S>(entries: I) -> Self
    where
        I: IntoIterator<Item = (S, InventoryEntry)>,
        S: Into<String>,
    {
        Self {
            entries: entries
                .into_iter()
                .map(|(check_id, entry)| (check_id.into(), entry))
                .collect(),
        }
    }

    /// Every recorded check, sorted, for storage.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &InventoryEntry)> {
        self.entries
            .iter()
            .map(|(check_id, entry)| (check_id.as_str(), entry))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Resolve a check id, falling back to the longest family prefix that
    /// covers it. Mirrors the manifest's own lookup, because a dynamic id
    /// (`accessibility.axe.label`) is governed by its family's row.
    pub fn lookup(&self, check_id: &str) -> Option<&InventoryEntry> {
        if let Some(exact) = self.entries.get(check_id) {
            return Some(exact);
        }
        self.entries
            .iter()
            .filter(|(prefix, entry)| entry.family && check_id.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, entry)| entry)
    }
}

/// One reading's provenance: what the build stated, and what it could produce.
#[derive(Debug, Clone, Copy)]
pub struct ObservationBasis<'a> {
    pub stamp: &'a ReleaseStamp,
    pub inventory: &'a CheckInventory,
}

/// Whether two readings of one check may be compared, and when they may not,
/// what moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparability {
    /// Same check, same meaning, same relevant runtime facts. A difference
    /// between the two readings is a difference in the thing being read.
    Comparable,
    /// The later build could produce this check and the earlier one could not.
    /// Its appearance is the scanner growing, never the site regressing.
    NewCheck,
    /// The earlier build could produce this check and the later one cannot.
    /// Its disappearance is the scanner shrinking, never the site improving.
    Retired,
    /// Both builds have the check and its meaning changed between them.
    DetectorChanged,
    /// Same meaning, but a runtime fact the check's verdict depends on moved.
    ProfileChanged(CompareDimension),
    /// Neither recorded build produces this check.
    Unregistered,
    /// One side's provenance is not on record. Nothing can be concluded, and
    /// concluding anyway is what this module exists to stop.
    Unattested,
}

impl Comparability {
    /// Whether a difference between the two readings may be attributed to the
    /// thing being read.
    pub fn is_comparable(&self) -> bool {
        matches!(self, Self::Comparable)
    }

    /// A stable identifier for logs and stored records.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Comparable => "comparable",
            Self::NewCheck => "new_check",
            Self::Retired => "retired",
            Self::DetectorChanged => "detector_changed",
            Self::ProfileChanged(_) => "profile_changed",
            Self::Unregistered => "unregistered",
            Self::Unattested => "unattested",
        }
    }
}

/// Compare one check across two readings.
///
/// Existence, contract, and runtime dimensions are evaluated in that order.
/// Dimensions come from the later manifest, which defines the current contract.
pub fn comparability(
    check_id: &str,
    before: Option<ObservationBasis<'_>>,
    after: Option<ObservationBasis<'_>>,
) -> Comparability {
    let (Some(before), Some(after)) = (before, after) else {
        return Comparability::Unattested;
    };
    match (
        before.inventory.lookup(check_id),
        after.inventory.lookup(check_id),
    ) {
        // Engine provenance cannot classify non-engine signals.
        (None, None) => Comparability::Unregistered,
        (None, Some(_)) => Comparability::NewCheck,
        (Some(_), None) => Comparability::Retired,
        (Some(earlier), Some(later)) => {
            match (&earlier.contract, &later.contract) {
                (Some(earlier_contract), Some(later_contract)) => {
                    if earlier_contract != later_contract {
                        return Comparability::DetectorChanged;
                    }
                }
                // A check that was versioned and now is not (or the reverse)
                // is a change in what we can promise about it, which is a
                // change in the promise.
                (Some(_), None) | (None, Some(_)) => return Comparability::DetectorChanged,
                (None, None) => {}
            }
            for dimension in &later.compare_on {
                if before.stamp.execution.dimension(*dimension)
                    != after.stamp.execution.dimension(*dimension)
                {
                    return Comparability::ProfileChanged(*dimension);
                }
            }
            Comparability::Comparable
        }
    }
}
