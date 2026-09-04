//! Query-time issue correlation and causal enrichment.

pub mod anomaly;
pub mod backfill;
pub mod causal_graph;
pub mod cross_env;
pub mod cross_page;
pub mod cross_project;
pub mod enrichments;
pub mod fix_locations;
pub mod integration_hints;
pub mod observations;
pub mod preview;
pub mod resolver;
pub mod signal_mapping;

pub use causal_graph::{resolve_likely_causes, CausalLink, Confidence, LikelyCause, CAUSAL_LINKS};
pub use fix_locations::resolve_fix_locations;
pub use integration_hints::{
    resolve_integration_suggestions, IntegrationHint, IntegrationSuggestion, INTEGRATION_HINTS,
};
pub use resolver::enrich_issue_groups;
pub use signal_mapping::{
    live_source_signals_for_check_id, resolve_check_id, source_signals_for_check_id,
    web_scan_check_id, SignalMapping, SIGNAL_MAPPINGS,
};
