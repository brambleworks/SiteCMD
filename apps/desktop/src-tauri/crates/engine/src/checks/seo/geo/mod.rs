//! Portable GEO (generative-engine optimization) checks: the sync page
//! analyses plus the probe-driven verdicts (llms.txt, AI crawler policy,
//! sitemap freshness) graded from runtime-supplied fetch outcomes.

pub mod ai_crawlers;
pub mod llms_txt;
pub mod metadata;
pub mod sitemap_freshness;
pub mod structure;

pub use metadata::{
    CitationMetaCheck, ContentFreshnessCheck, FaqSchemaCheck, OrganizationIdentityCheck,
};
pub use structure::{
    js_shell_signature, JsOnlyContentCheck, SemanticHtmlCheck, SourceCitationsCheck,
};
