//! Strict wire schema for declarative catalog packs.
//!
//! Unknown fields are rejected, and guide text is never executed or resolved as
//! a path, URL, or module.

use serde::{Deserialize, Serialize};

use crate::constants::{
    CATALOG_MAX_ENTRIES, CATALOG_MAX_FRAMEWORK_VARIANTS, CATALOG_MAX_KEY_CHARS,
    CATALOG_MAX_STEPS_PER_GUIDE, CATALOG_MAX_STEP_CHARS,
};

/// The schema version this engine understands. A pack declaring anything else
/// is rejected rather than best-effort parsed.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// How much work a fix is expected to be. A closed set, so a pack cannot invent
/// a variant the UI has no rendering for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Quick,
    Moderate,
    Involved,
}

/// One remediation guide: default steps plus optional framework-specific
/// variants, keyed by a normalized framework name the engine detected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuideEntry {
    pub effort: Effort,
    pub effort_minutes: u32,
    pub default: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frameworks: Option<std::collections::BTreeMap<String, Vec<String>>>,
}

/// A complete catalog pack. `guides` is keyed by check id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPack {
    pub schema_version: u32,
    /// Human-readable release label, e.g. "2026.07.26".
    pub catalog_version: String,
    /// Monotonic release sequence used for rollback protection.
    pub release_sequence: u64,
    pub published_at: String,
    /// Engine versions below this cannot render the pack correctly.
    pub minimum_engine_version: String,
    pub guides: std::collections::BTreeMap<String, GuideEntry>,
}

/// Specific reason a catalog pack was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    #[error("catalog schema version {found} is not supported (this engine reads {SUPPORTED_SCHEMA_VERSION})")]
    UnsupportedSchemaVersion { found: u32 },
    #[error("catalog carries {found} guide entries, above the {CATALOG_MAX_ENTRIES} limit")]
    TooManyEntries { found: usize },
    #[error("catalog key {key:?} is longer than {CATALOG_MAX_KEY_CHARS} characters")]
    KeyTooLong { key: String },
    #[error("guide {check_id:?} has {found} steps, above the {CATALOG_MAX_STEPS_PER_GUIDE} limit")]
    TooManySteps { check_id: String, found: usize },
    #[error("guide {check_id:?} has a step longer than {CATALOG_MAX_STEP_CHARS} characters")]
    StepTooLong { check_id: String },
    #[error(
        "guide {check_id:?} has {found} framework variants, above the {CATALOG_MAX_FRAMEWORK_VARIANTS} limit"
    )]
    TooManyFrameworks { check_id: String, found: usize },
    #[error("guide {check_id:?} has no remediation steps")]
    EmptyGuide { check_id: String },
    #[error("catalog {field} is longer than {CATALOG_MAX_KEY_CHARS} characters")]
    MetadataTooLong { field: &'static str },
}

impl CatalogPack {
    /// Enforce every capability limit. Runs after signature verification and
    /// before activation, so a pack that is authentically signed but malformed
    /// still cannot reach the renderer.
    pub fn validate(&self) -> Result<(), SchemaError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(SchemaError::UnsupportedSchemaVersion {
                found: self.schema_version,
            });
        }
        if self.guides.len() > CATALOG_MAX_ENTRIES {
            return Err(SchemaError::TooManyEntries {
                found: self.guides.len(),
            });
        }
        // Bound catalog_version because later requests echo it in the query and
        // must remain able to fetch a replacement pack.
        if self.catalog_version.chars().count() > CATALOG_MAX_KEY_CHARS {
            return Err(SchemaError::MetadataTooLong {
                field: "catalog_version",
            });
        }
        if self.minimum_engine_version.chars().count() > CATALOG_MAX_KEY_CHARS {
            return Err(SchemaError::MetadataTooLong {
                field: "minimum_engine_version",
            });
        }

        for (check_id, guide) in &self.guides {
            if check_id.chars().count() > CATALOG_MAX_KEY_CHARS {
                return Err(SchemaError::KeyTooLong {
                    key: check_id.clone(),
                });
            }
            guide.validate(check_id)?;
        }
        Ok(())
    }
}

impl GuideEntry {
    fn validate(&self, check_id: &str) -> Result<(), SchemaError> {
        // A guide with no steps renders as an empty panel, which reads to the
        // user as "SiteCMD has nothing" rather than as a broken download.
        if self.default.is_empty() {
            return Err(SchemaError::EmptyGuide {
                check_id: check_id.to_string(),
            });
        }
        check_steps(check_id, &self.default)?;

        let Some(frameworks) = &self.frameworks else {
            return Ok(());
        };
        if frameworks.len() > CATALOG_MAX_FRAMEWORK_VARIANTS {
            return Err(SchemaError::TooManyFrameworks {
                check_id: check_id.to_string(),
                found: frameworks.len(),
            });
        }
        for (framework, steps) in frameworks {
            if framework.chars().count() > CATALOG_MAX_KEY_CHARS {
                return Err(SchemaError::KeyTooLong {
                    key: framework.clone(),
                });
            }
            // A variant overrides the default outright, so an empty one is
            // worse than no variant at all: `{"next": []}` left every Next.js
            // user with a blank guide while every other stack read fine.
            if steps.is_empty() {
                return Err(SchemaError::EmptyGuide {
                    check_id: format!("{check_id} ({framework})"),
                });
            }
            check_steps(check_id, steps)?;
        }
        Ok(())
    }
}

fn check_steps(check_id: &str, steps: &[String]) -> Result<(), SchemaError> {
    if steps.len() > CATALOG_MAX_STEPS_PER_GUIDE {
        return Err(SchemaError::TooManySteps {
            check_id: check_id.to_string(),
            found: steps.len(),
        });
    }
    // Count characters, not bytes: the limit exists to bound what reaches the
    // renderer, and a multi-byte step is not automatically a hostile one.
    if steps
        .iter()
        .any(|s| s.chars().count() > CATALOG_MAX_STEP_CHARS)
    {
        return Err(SchemaError::StepTooLong {
            check_id: check_id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
