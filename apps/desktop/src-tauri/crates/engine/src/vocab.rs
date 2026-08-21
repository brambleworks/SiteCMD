//! Shared scan vocabulary for desktop, CLI, and hosted runtimes.

use serde::{Deserialize, Serialize};

/// Status of an individual check
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Fail,
    Warn,
    Skipped,
}

impl CheckStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Warn => "warn",
            Self::Skipped => "skipped",
        }
    }
}

impl std::str::FromStr for CheckStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "warn" => Ok(Self::Warn),
            "skipped" => Ok(Self::Skipped),
            _ => Err(format!("unknown CheckStatus: {}", s)),
        }
    }
}

/// Issue severity
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

impl Severity {
    /// Declaration order = severity order (most severe first).
    pub const ALL: [Self; 4] = [Self::Critical, Self::High, Self::Medium, Self::Low];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
        }
    }

    pub fn sort_rank(self) -> u8 {
        match self {
            Self::Critical => 0,
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 3,
        }
    }

    pub fn impact_rank(self) -> u8 {
        match self {
            Self::Critical => 4,
            Self::High => 3,
            Self::Medium => 2,
            Self::Low => 1,
        }
    }

    pub fn impact_rank_for_label(value: &str) -> u8 {
        value.parse::<Self>().map(Self::impact_rank).unwrap_or(0)
    }

    pub fn sort_rank_for_label(value: &str) -> u8 {
        value.parse::<Self>().map(Self::sort_rank).unwrap_or(4)
    }

    pub fn label_for_impact_rank(rank: u8) -> &'static str {
        match rank {
            4 => Self::Critical.as_str(),
            3 => Self::High.as_str(),
            2 => Self::Medium.as_str(),
            _ => Self::Low.as_str(),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            _ => Err(format!("unknown Severity: {}", s)),
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How certain SiteCMD is that the reported condition was observed or
/// reliably inferred. Confidence grades the evidence, not whether the same
/// remediation is appropriate in every project context.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "snake_case")]
pub enum IssueConfidence {
    Confirmed,
    #[default]
    High,
    NeedsReview,
}

impl IssueConfidence {
    /// Mirrors canConfidenceTriggerScoreCap in src/lib/issue-confidence.ts: a
    /// needs-review finding (possible false positive) must never slam the
    /// score into the red via the exploitable cap.
    pub fn can_trigger_score_cap(self) -> bool {
        !matches!(self, IssueConfidence::NeedsReview)
    }

    /// Stable column/wire form; matches the serde snake_case spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::High => "high",
            Self::NeedsReview => "needs_review",
        }
    }
}

impl std::str::FromStr for IssueConfidence {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "confirmed" => Ok(Self::Confirmed),
            "high" => Ok(Self::High),
            "needs_review" => Ok(Self::NeedsReview),
            _ => Err(format!("unknown IssueConfidence: {}", s)),
        }
    }
}

/// Scan category
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "lowercase")]
pub enum ScanCategory {
    Security,
    Performance,
    Seo,
    Accessibility,
    Compliance,
    Config,
    Polish,
}

impl ScanCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Performance => "performance",
            Self::Seo => "seo",
            Self::Accessibility => "accessibility",
            Self::Compliance => "compliance",
            Self::Config => "config",
            Self::Polish => "polish",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            Self::Security => "Security",
            Self::Performance => "Performance",
            Self::Seo => "SEO",
            Self::Accessibility => "Accessibility",
            Self::Compliance => "Compliance",
            Self::Config => "Config",
            Self::Polish => "Polish",
        }
    }
}

impl std::str::FromStr for ScanCategory {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "security" => Ok(Self::Security),
            "performance" => Ok(Self::Performance),
            "seo" => Ok(Self::Seo),
            "accessibility" => Ok(Self::Accessibility),
            "compliance" => Ok(Self::Compliance),
            "config" => Ok(Self::Config),
            "polish" => Ok(Self::Polish),
            _ => Err(format!("unknown ScanCategory: {}", s)),
        }
    }
}

/// Result from a single check
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub check_id: String,
    pub category: ScanCategory,
    pub title: String,
    pub description: String,
    pub status: CheckStatus,
    pub severity: Severity,
    pub fix_prompt: Option<String>,
    pub manual_fix: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub raw_data: Option<serde_json::Value>,
    #[serde(default)]
    pub confidence: IssueConfidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub confidence_reason: Option<String>,
    /// One-line business impact statement for critical/high issues.
    /// Answers "what breaks if I ignore this?"
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub why_it_matters: Option<String>,
}

/// The single lifecycle-status vocabulary for unified issues.
///
/// Serialized into the constrained issue-state column and generated IPC types.
/// Every scorer uses `is_inactive_for_scoring` as its active-set predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    New,
    Snoozed,
    Ignored,
    Blocked,
    Verified,
    Regressed,
}

impl IssueStatus {
    pub const ALL: [IssueStatus; 6] = [
        IssueStatus::New,
        IssueStatus::Snoozed,
        IssueStatus::Ignored,
        IssueStatus::Blocked,
        IssueStatus::Verified,
        IssueStatus::Regressed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            IssueStatus::New => "new",
            IssueStatus::Snoozed => "snoozed",
            IssueStatus::Ignored => "ignored",
            IssueStatus::Blocked => "blocked",
            IssueStatus::Verified => "verified",
            IssueStatus::Regressed => "regressed",
        }
    }

    /// Statuses that remove an issue from the active list and the score.
    /// `Regressed` stays active: a verified issue that re-failed must count
    /// again. Mirrored by the MCP server's DISMISSED_STATUSES (guardrail-pinned)
    /// and the frontend's active-issue filter (generated-manifest-pinned).
    pub fn is_inactive_for_scoring(self) -> bool {
        matches!(
            self,
            IssueStatus::Snoozed
                | IssueStatus::Ignored
                | IssueStatus::Blocked
                | IssueStatus::Verified
        )
    }

    /// Resolve expired snoozes back to `New` at `now_ms`.
    pub fn effective(self, snooze_until: Option<i64>, now_ms: i64) -> IssueStatus {
        if self == IssueStatus::Snoozed && snooze_until.is_some_and(|until| until <= now_ms) {
            IssueStatus::New
        } else {
            self
        }
    }
}

impl std::fmt::Display for IssueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for IssueStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        IssueStatus::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
            .ok_or_else(|| format!("unknown IssueStatus: {}", value))
    }
}

/// Evidence source for a verified lifecycle row.
/// User claims remain distinct from scan-observed absence in local and
/// connected lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifiedBy {
    /// The user said the issue was fixed. Nothing looked.
    UserClaim,
    /// A local scan re-ran the check and stopped reporting the issue.
    LocalScan,
}

impl VerifiedBy {
    pub const ALL: [VerifiedBy; 2] = [VerifiedBy::UserClaim, VerifiedBy::LocalScan];

    pub fn as_str(self) -> &'static str {
        match self {
            VerifiedBy::UserClaim => "user_claim",
            VerifiedBy::LocalScan => "local_scan",
        }
    }

    /// Whether observation proved the issue absent. Only observed absence may
    /// make a later recurrence a regression.
    pub fn proves_absence(self) -> bool {
        matches!(self, VerifiedBy::LocalScan)
    }
}

impl std::fmt::Display for VerifiedBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for VerifiedBy {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        VerifiedBy::ALL
            .into_iter()
            .find(|verified_by| verified_by.as_str() == value)
            .ok_or_else(|| format!("unknown VerifiedBy: {}", value))
    }
}

#[cfg(test)]
mod severity_tests {
    use super::Severity;

    #[test]
    fn severity_string_helpers_are_canonical() {
        assert_eq!(Severity::Critical.as_str(), "critical");
        assert_eq!(Severity::High.label(), "High");
        assert!(Severity::Critical.sort_rank() < Severity::High.sort_rank());
        assert!(Severity::Critical.impact_rank() > Severity::High.impact_rank());
        assert_eq!(Severity::impact_rank_for_label("critical"), 4);
        assert_eq!(Severity::sort_rank_for_label("critical"), 0);
        assert_eq!(Severity::sort_rank_for_label("unknown"), 4);
        assert_eq!(Severity::label_for_impact_rank(3), "high");
    }
}

#[cfg(test)]
mod scan_category_tests {
    use super::ScanCategory;

    #[test]
    fn scan_category_string_helpers_are_canonical() {
        assert_eq!(ScanCategory::Security.as_str(), "security");
        assert_eq!(ScanCategory::Seo.display_label(), "SEO");
        assert_eq!(ScanCategory::Config.as_str(), "config");
    }
}
