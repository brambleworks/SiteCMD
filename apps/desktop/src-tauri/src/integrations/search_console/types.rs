use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlInspectionResult {
    pub page_url: String,
    pub verdict: String,                // "PASS" | "PARTIAL" | "FAIL" | "NEUTRAL"
    pub coverage_state: String, // "Submitted and indexed", "Crawled - currently not indexed", etc.
    pub indexing_state: Option<String>, // "INDEXED" | "INDEXING_ALLOWED" | "BLOCKED_BY_ROBOTS_TXT" | ...
    pub page_fetch_state: Option<String>,
    pub robots_txt_state: Option<String>,
    pub last_crawl_time: Option<String>,
    pub mobile_friendly: Option<bool>,
    pub mobile_usability_issues: Vec<String>,
    pub canonical_inspection: Option<CanonicalInspection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalInspection {
    pub user_canonical: Option<String>,
    pub google_canonical: Option<String>,
    pub mismatch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexCoverageIssue {
    pub page_url: String,
    pub reason: String, // "not-indexed" | "crawl-error" | "blocked-by-robots" | "canonical-mismatch" | "duplicate-no-canonical" | "mobile-viewport" | "text-too-small" | "touch-target-size" | "content-wider-than-screen"
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchConsoleData {
    pub total_clicks: u64,
    pub total_impressions: u64,
    pub average_ctr: f64,
    pub average_position: f64,
    pub top_queries: Vec<SearchQuery>,
    pub top_pages: Vec<SearchPage>,
    pub daily: Vec<SearchDailyPoint>,
    pub devices: Vec<SearchDevice>,
    pub countries: Vec<SearchCountry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub clicks: u64,
    pub impressions: u64,
    pub ctr: f64,
    pub position: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchPage {
    pub page: String,
    pub clicks: u64,
    pub impressions: u64,
    pub ctr: f64,
    pub position: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchDailyPoint {
    pub date: String,
    pub clicks: u64,
    pub impressions: u64,
    pub ctr: f64,
    pub position: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchDevice {
    pub device: String,
    pub clicks: u64,
    pub impressions: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchCountry {
    pub country: String,
    pub clicks: u64,
    pub impressions: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GSCSite {
    pub site_url: String,
    pub permission: String,
}

/// Represents a query that has regressed between two consecutive windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRegression {
    pub query: String,
    pub previous_impressions: i64,
    pub current_impressions: i64,
    pub previous_clicks: i64,
    pub current_clicks: i64,
    pub previous_ctr: f64,
    pub current_ctr: f64,
    pub previous_position: f64,
    pub current_position: f64,
    pub detected_at: i64, // unix ms
}
