//! Canonical, bounded projections for verified-good fact families.
//!
//! Projections use allowlists and report overflow so truncated sets never appear complete.

use super::{ProfileField, PROFILE_VERSION};
use crate::checks::floor_char_boundary;
use crate::checks::security::tls::TlsFacts;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Certificate subject/SAN names kept per record (transient projection v1).
const MAX_CERTIFICATE_NAMES: usize = 100;
/// Byte bound on any single name string.
const MAX_NAME_BYTES: usize = 256;
/// Byte bound on one header field line.
pub(super) const MAX_HEADER_VALUE_BYTES: usize = 2_048;
/// Third-party origins kept per record.
pub(super) const MAX_ORIGINS: usize = 128;
/// Mail exchange hosts kept per record.
const MAX_MX_HOSTS: usize = 32;
/// Routes kept per record; the scope resource's own bound.
const MAX_ROUTES: usize = crate::scope::SCOPE_WIRE_LIMIT;
/// Byte bound on a DNS policy string (SPF, DMARC).
const MAX_POLICY_BYTES: usize = 512;

/// Headers permitted in stored profiles. Unlisted values cannot enter the projection.
pub const SECURITY_HEADER_ALLOWLIST: &[&str] = &[
    "cache-control",
    "content-security-policy",
    "cross-origin-embedder-policy",
    "cross-origin-opener-policy",
    "cross-origin-resource-policy",
    "permissions-policy",
    "referrer-policy",
    "strict-transport-security",
    "x-content-type-options",
    "x-frame-options",
];

/// The DNS TXT prefixes a policy string may start with. Arbitrary TXT content
/// never rides: TXT records routinely hold third-party verification secrets.
const TXT_POLICY_PREFIXES: &[&str] = &["v=spf1", "v=DMARC1"];

/// A sorted, deduplicated, bounded string set that always says how much it
/// dropped. The count rides in the serialization, so a bound can never make
/// two different sets look identical.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedSet {
    pub values: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub overflow: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

impl BoundedSet {
    /// Canonicalize: trim, drop empties, bound each entry, sort, dedup, then
    /// bound the set itself and record what fell off the end.
    pub fn new(values: impl IntoIterator<Item = String>, limit: usize) -> Self {
        let mut sorted: Vec<String> = values
            .into_iter()
            .map(|value| bounded_string(value.trim(), MAX_NAME_BYTES).0)
            .filter(|value| !value.is_empty())
            .collect();
        sorted.sort_unstable();
        sorted.dedup();
        let overflow = sorted.len().saturating_sub(limit);
        sorted.truncate(limit);
        Self {
            values: sorted,
            overflow,
        }
    }

    /// True when every value here is also in `other`. Overflow makes the
    /// answer unknowable, so an overflowed set is never treated as contained:
    /// a bound narrows what a record can prove, never what it claims.
    pub(super) fn is_subset_of(&self, other: &Self) -> bool {
        if self.overflow > 0 || other.overflow > 0 {
            return false;
        }
        let theirs: BTreeSet<&str> = other.values.iter().map(String::as_str).collect();
        self.values
            .iter()
            .all(|value| theirs.contains(value.as_str()))
    }

    /// Merge bounded sets, retaining the larger overflow count as a safe lower bound.
    pub(super) fn union(&self, other: &Self, limit: usize) -> Self {
        let merged = self.values.iter().chain(other.values.iter()).cloned();
        let mut union = Self::new(merged, limit);
        union.overflow += self.overflow.max(other.overflow);
        union
    }
}

/// Truncate at a UTF-8 boundary, reporting whether the bound bit.
fn bounded_string(value: &str, limit: usize) -> (String, bool) {
    if value.len() <= limit {
        return (value.to_string(), false);
    }
    let end = floor_char_boundary(value, limit);
    (value[..end].to_string(), true)
}

/// Collapse internal whitespace runs to one space and trim the ends, so two
/// servers that differ only in spacing produce one value.
fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Certificate issuer and SANs; renewal dates are scored separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateIdentity {
    pub subject_names: BoundedSet,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
}

impl CertificateIdentity {
    /// Returns `None` when the captured facts contain no identity.
    pub fn from_tls_facts(facts: &TlsFacts) -> Option<Self> {
        let subject_names = BoundedSet::new(facts.subject_names.clone(), MAX_CERTIFICATE_NAMES);
        let issuer = facts
            .issuer
            .as_deref()
            .map(|issuer| bounded_string(collapse_whitespace(issuer).as_str(), MAX_NAME_BYTES).0)
            .filter(|issuer| !issuer.is_empty());
        if subject_names.values.is_empty() && issuer.is_none() {
            return None;
        }
        Some(Self {
            subject_names,
            issuer,
        })
    }
}

/// One header field line, kept separate from its siblings. Field lines are
/// never joined: browsers enforce multiple Content-Security-Policy lines
/// independently, so joining is lossy exactly where it matters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderLine {
    pub value: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

pub(super) fn is_false(value: &bool) -> bool {
    !*value
}

/// The allowlisted response headers of a route, in received order per header.
/// An absent header is an absent key, never an empty list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityHeaderProfile {
    pub headers: BTreeMap<String, Vec<HeaderLine>>,
}

impl SecurityHeaderProfile {
    /// Project a response header map through the allowlist.
    pub fn from_headers(headers: &http::HeaderMap) -> Self {
        let mut projected: BTreeMap<String, Vec<HeaderLine>> = BTreeMap::new();
        for name in SECURITY_HEADER_ALLOWLIST {
            let lines: Vec<HeaderLine> = headers
                .get_all(*name)
                .iter()
                .filter_map(|value| value.to_str().ok())
                .map(|value| {
                    let (value, truncated) =
                        bounded_string(collapse_whitespace(value).as_str(), MAX_HEADER_VALUE_BYTES);
                    HeaderLine { value, truncated }
                })
                .collect();
            if !lines.is_empty() {
                projected.insert((*name).to_string(), lines);
            }
        }
        Self { headers: projected }
    }
}

/// The third-party origins a page loads from, normalized by the URL parser's
/// own origin serialization (lowercase scheme and host, punycode, default
/// ports omitted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginSet {
    pub origins: BoundedSet,
}

/// Resource-loading tags and their target attributes; navigation links are excluded.
const RESOURCE_TAGS: &[(&str, &str)] = &[
    ("audio", "src"),
    ("embed", "src"),
    ("form", "action"),
    ("iframe", "src"),
    ("img", "src"),
    ("link", "href"),
    ("object", "data"),
    ("script", "src"),
    ("source", "src"),
    ("track", "src"),
    ("video", "src"),
];

impl OriginSet {
    /// Extract the cross-origin resource targets of a document.
    pub fn from_document(page_url: &url::Url, body: &str, body_lower: &str) -> Self {
        let page_origin = page_url.origin();
        let origins = RESOURCE_TAGS
            .iter()
            .flat_map(|(tag_name, attribute)| {
                crate::checks::html_attrs::tag_slices(body, body_lower, tag_name)
                    .into_iter()
                    .filter_map(|tag| crate::checks::html_attrs::attr_value(tag, attribute))
            })
            .filter_map(|reference| page_url.join(reference.trim()).ok())
            .filter(|resolved| matches!(resolved.scheme(), "http" | "https"))
            .map(|resolved| resolved.origin())
            .filter(|origin| *origin != page_origin)
            .map(|origin| origin.ascii_serialization());
        Self {
            origins: BoundedSet::new(origins, MAX_ORIGINS),
        }
    }

    /// Build from origins an adapter already resolved.
    pub fn from_origins(origins: impl IntoIterator<Item = String>) -> Self {
        Self {
            origins: BoundedSet::new(origins, MAX_ORIGINS),
        }
    }

    /// Merge page-level observations into one site-level origin set while
    /// preserving the bounded-set overflow marker.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            origins: self.origins.union(&other.origins, MAX_ORIGINS),
        }
    }
}

/// Mail and certificate-authority posture in DNS. Address records are absent
/// on purpose: they rotate as normal CDN operation, and a takeover signal is
/// the dangling-CNAME verdict's job, not a baseline's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsPosture {
    pub mx_hosts: BoundedSet,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cname_target: Option<String>,
    pub caa_present: bool,
    /// The SPF policy string, kept only because it matches an allowlisted
    /// prefix. Arbitrary TXT never rides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmarc: Option<String>,
}

impl DnsPosture {
    /// Build a posture record, dropping any TXT string that is not an
    /// allowlisted policy.
    pub fn new(
        mx_hosts: impl IntoIterator<Item = String>,
        cname_target: Option<String>,
        caa_present: bool,
        txt_records: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut spf = None;
        let mut dmarc = None;
        for record in txt_records {
            let record = collapse_whitespace(&record);
            let Some(prefix) = TXT_POLICY_PREFIXES.iter().find(|prefix| {
                record
                    .to_ascii_lowercase()
                    .starts_with(&prefix.to_ascii_lowercase())
            }) else {
                continue;
            };
            let (policy, _) = bounded_string(&record, MAX_POLICY_BYTES);
            if prefix.eq_ignore_ascii_case("v=spf1") {
                spf.get_or_insert(policy);
            } else {
                dmarc.get_or_insert(policy);
            }
        }
        Self {
            mx_hosts: BoundedSet::new(mx_hosts, MAX_MX_HOSTS),
            cname_target: cname_target
                .map(|target| bounded_string(target.trim(), MAX_NAME_BYTES).0)
                .filter(|target| !target.is_empty()),
            caa_present,
            spf,
            dmarc,
        }
    }
}

/// The routes known to exist on the site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteSet {
    pub routes: BoundedSet,
}

impl RouteSet {
    pub fn new(routes: impl IntoIterator<Item = String>) -> Self {
        Self {
            routes: BoundedSet::new(routes, MAX_ROUTES),
        }
    }
}

/// The canonical value of one fact family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "field", rename_all = "snake_case")]
pub enum FieldValue {
    Certificate(CertificateIdentity),
    SecurityHeaders(SecurityHeaderProfile),
    ThirdPartyOrigins(OriginSet),
    DnsPosture(DnsPosture),
    RouteSet(RouteSet),
}

impl FieldValue {
    pub fn field(&self) -> ProfileField {
        match self {
            Self::Certificate(_) => ProfileField::Certificate,
            Self::SecurityHeaders(_) => ProfileField::SecurityHeaders,
            Self::ThirdPartyOrigins(_) => ProfileField::ThirdPartyOrigins,
            Self::DnsPosture(_) => ProfileField::DnsPosture,
            Self::RouteSet(_) => ProfileField::RouteSet,
        }
    }

    /// The value's identity. Acceptance guards on this string, so a delayed
    /// click cannot bless a value the person never saw.
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(PROFILE_VERSION.to_string().as_bytes());
        hasher.update([0x1F]);
        hasher.update(
            serde_json::to_string(self)
                .expect("profile value serializes")
                .as_bytes(),
        );
        hex::encode(&hasher.finalize()[..8])
    }

    /// Compare an observation against this good value.
    pub(super) fn compare(&self, observed: &FieldValue) -> Comparison {
        if self.field() != observed.field() {
            return Comparison::Changed(observed.clone());
        }
        match (self, observed) {
            (Self::ThirdPartyOrigins(good), Self::ThirdPartyOrigins(seen)) => {
                if seen.origins.is_subset_of(&good.origins) {
                    if seen == good {
                        Comparison::Match
                    } else {
                        Comparison::Thinner
                    }
                } else {
                    Comparison::Changed(Self::ThirdPartyOrigins(OriginSet {
                        origins: good.origins.union(&seen.origins, MAX_ORIGINS),
                    }))
                }
            }
            (Self::RouteSet(good), Self::RouteSet(seen)) => {
                if seen.routes.is_subset_of(&good.routes) {
                    if seen == good {
                        Comparison::Match
                    } else {
                        Comparison::Thinner
                    }
                } else {
                    Comparison::Changed(Self::RouteSet(RouteSet {
                        routes: good.routes.union(&seen.routes, MAX_ROUTES),
                    }))
                }
            }
            _ if self == observed => Comparison::Match,
            _ => Comparison::Changed(observed.clone()),
        }
    }
}

pub(super) enum Comparison {
    Match,
    /// A growth-only family observed a strict subset of good. This proves no
    /// new drift, but it cannot prove an existing drift recovered.
    Thinner,
    /// The value that should be recorded as the drift, which for a
    /// growth-only family is the union rather than the thinner observation.
    Changed(FieldValue),
}
