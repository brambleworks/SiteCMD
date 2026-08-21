/** Analytics response types mirroring the Rust integration structs. */

interface PlausibleAggregate {
  visitors: number;
  pageviews: number;
  bounce_rate: number;
  visit_duration: number;
}

interface TimeseriesPoint {
  date: string;
  visitors: number;
  pageviews: number;
  bounce_rate: number;
  visit_duration: number;
}

interface PlausibleTopPage {
  page: string;
  visitors: number;
}

interface PlausibleTopSource {
  source: string;
  visitors: number;
}

interface PlausibleCountry {
  country: string;
  visitors: number;
}

interface PlausibleDevice {
  device: string;
  visitors: number;
}

interface PlausibleBrowser {
  browser: string;
  visitors: number;
}

interface PlausibleTimeseries {
  period: string;
  points: TimeseriesPoint[];
  aggregate: PlausibleAggregate;
  top_pages: PlausibleTopPage[];
  top_sources: PlausibleTopSource[];
  countries: PlausibleCountry[];
  devices: PlausibleDevice[];
  browsers: PlausibleBrowser[];
}

interface CloudflareData {
  requests_total: number;
  requests_cached: number;
  cache_hit_rate: number;
  bandwidth_total: number;
  bandwidth_cached: number;
  threats_blocked: number;
  page_views: number;
  unique_visitors: number;
}

interface ResponseTimePoint {
  datetime: number;
  value: number;
}

interface UptimeLogEntry {
  log_type: number; // 1=down, 2=up, 98=started, 99=paused
  type_text: string;
  datetime: string;
  duration: number; // seconds
  reason_code?: string;
  reason_detail?: string;
}

interface MonitorData {
  friendly_name: string;
  url: string;
  status: number; // 0=paused, 1=not checked, 2=up, 8=seems down, 9=down
  status_text: string;
  uptime_ratio: number;
  average_response: number;
  last_downtime?: string;
  response_times: ResponseTimePoint[];
  logs: UptimeLogEntry[];
}

interface UptimeRobotData {
  monitors: MonitorData[];
}

interface GA4TopPage {
  page: string;
  views: number;
}

interface GA4TopSource {
  source: string;
  users: number;
}

interface GA4DailyPoint {
  date: string;
  users: number;
  sessions: number;
  pageviews: number;
}

interface GA4Data {
  active_users: number;
  sessions: number;
  pageviews: number;
  bounce_rate: number;
  avg_session_duration: number;
  top_pages: GA4TopPage[];
  top_sources: GA4TopSource[];
  daily: GA4DailyPoint[];
}

export interface SearchQuery {
  query: string;
  clicks: number;
  impressions: number;
  ctr: number;
  position: number;
}

export interface SearchPage {
  page: string;
  clicks: number;
  impressions: number;
  ctr: number;
  position: number;
}

interface SearchDailyPoint {
  date: string;
  clicks: number;
  impressions: number;
  ctr: number;
  position: number;
}

export interface SearchDevice {
  device: string;
  clicks: number;
  impressions: number;
}

export interface SearchConsoleData {
  total_clicks: number;
  total_impressions: number;
  average_ctr: number;
  average_position: number;
  top_queries: SearchQuery[];
  top_pages: SearchPage[];
  daily: SearchDailyPoint[];
  devices: SearchDevice[];
}

interface BingDailyStat {
  date: string;
  clicks: number;
  impressions: number;
}

export interface BingQueryStat {
  query: string;
  clicks: number;
  impressions: number;
  avg_position: number;
}

export interface BingPageStat {
  url: string;
  clicks: number;
  impressions: number;
  avg_position: number;
}

export interface BingSearchData {
  total_clicks: number;
  total_impressions: number;
  avg_position: number;
  daily_stats: BingDailyStat[];
  top_queries: BingQueryStat[];
  top_pages: BingPageStat[];
  crawl_errors: number;
}

export interface AnalyticsResponse {
  plausible?: PlausibleTimeseries;
  cloudflare?: CloudflareData;
  uptimerobot?: UptimeRobotData;
  google_analytics?: GA4Data;
  search_console?: SearchConsoleData;
  bing?: BingSearchData;
  plausible_error?: string;
  cloudflare_error?: string;
  uptimerobot_error?: string;
  google_analytics_error?: string;
  search_console_error?: string;
  bing_error?: string;
}

export interface GitHubData {
  repo: string;
  workflow_runs: WorkflowRun[];
  deployments: GitHubDeployment[];
  open_prs: PullRequest[];
}

export interface WorkflowRun {
  id: number;
  name: string;
  head_branch: string;
  head_sha: string;
  status: string;
  conclusion: string | null;
  run_number: number;
  created_at: string;
  updated_at: string;
  html_url: string;
  duration_seconds: number | null;
}

interface GitHubDeployment {
  id: number;
  environment: string;
  sha: string;
  description: string | null;
  status: string;
  created_at: string;
  creator: string;
}

export interface PullRequest {
  number: number;
  title: string;
  state: string;
  user: string;
  head_branch: string;
  created_at: string;
  updated_at: string;
  html_url: string;
  draft: boolean;
  additions: number;
  deletions: number;
  changed_files: number;
}
