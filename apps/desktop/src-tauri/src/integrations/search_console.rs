//! Google Search Console API client.

mod analytics;
mod inspection;
mod query_comparison;
mod sites;
mod types;

const GSC_API_URL: &str = "https://www.googleapis.com/webmasters/v3";

pub use analytics::fetch_analytics;
pub use inspection::{fetch_index_coverage_issues, fetch_url_inspection};
pub use query_comparison::fetch_query_comparison;
pub use sites::list_sites;
pub use types::{
    CanonicalInspection, GSCSite, IndexCoverageIssue, QueryRegression, SearchConsoleData,
    SearchCountry, SearchDailyPoint, SearchDevice, SearchPage, SearchQuery, UrlInspectionResult,
};

#[cfg(test)]
pub(crate) use analytics::{
    parse_aggregate_totals, parse_countries, parse_daily, parse_devices, parse_rows,
    parse_top_pages, parse_top_queries,
};
#[cfg(test)]
pub(crate) use inspection::{classify_inspection, parse_url_inspection};
#[cfg(test)]
pub(crate) use sites::parse_sites;

#[cfg(test)]
mod tests;
