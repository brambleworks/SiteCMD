//! Shared browser payloads and portable fact schemas.
//!
//! Runtime-specific browsers inject the same assets and grade returned facts in
//! the engine's accessibility and performance checks.

pub mod facts;
pub mod payload;

pub use facts::{
    parse_axe_report, AxeNodeEvidence, AxeReport, AxeViolation, CoreWebVitals, RuleOutcome,
};
pub use payload::{
    axe_run_script, AxeEvidenceCaps, AXE_CORE_VERSION, AXE_RESULT_GLOBAL, AXE_RUN_TAGS,
    CWV_OBSERVER_SCRIPT, CWV_READ_SCRIPT,
};

#[cfg(feature = "browser-payload")]
pub use payload::AXE_CORE_SCRIPT;
