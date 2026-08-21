//! Source-level checks for content structure, attribution metadata, optional
//! machine-readable conventions, and crawler policy. These checks do not
//! measure or promise inclusion, citation, ranking, or answer-system usage.

mod async_checks;
use sitecmd_engine::checks::seo::geo::{metadata, structure};

pub use async_checks::{AiCrawlerBlockingCheck, LlmsTxtCheck, SitemapFreshnessCheck};
pub use metadata::{
    CitationMetaCheck, ContentFreshnessCheck, FaqSchemaCheck, OrganizationIdentityCheck,
};
pub use structure::{JsOnlyContentCheck, SemanticHtmlCheck, SourceCitationsCheck};
