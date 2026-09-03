//! Shared browser payloads and portable fact schemas.
//!
//! Runtime-specific browsers inject the same assets and grade returned facts in
//! the engine's accessibility and performance checks.

pub mod facts;
pub mod identity;
pub mod payload;

pub use facts::{
    axe_report_from_value, AxeNodeEvidence, AxeReport, AxeViolation, CoreWebVitals, RuleOutcome,
};
pub use identity::{payload_document_url, AdmittedDocuments, DocumentMismatch};
pub use payload::{
    axe_run_script, AxeEvidenceCaps, AXE_CORE_VERSION, AXE_RESULT_GLOBAL, AXE_RUN_TAGS,
    CWV_OBSERVER_SCRIPT, CWV_READ_SCRIPT, CWV_RESULT_GLOBAL,
};

#[cfg(feature = "browser-payload")]
pub use payload::AXE_CORE_SCRIPT;
