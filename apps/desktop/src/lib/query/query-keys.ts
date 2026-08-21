/** Central query-key registry. Each domain exposes an `all` prefix and typed keys. */
export const queryKeys = {
  /** Project list used by the shell and project switcher. */
  projects: {
    all: ["projects"] as const,
    list: () => ["projects", "list"] as const,
  },
  /** Current Rust-computed SiteCMD Score for one project/environment. */
  currentScore: {
    all: ["currentScore"] as const,
    projectScope: (projectId: number) => ["currentScore", projectId] as const,
    forEnv: (projectId: number | null, envUrl: string) =>
      ["currentScore", projectId, envUrl] as const,
  },
  /** Canonical execution detail payload, addressable through any child run id. */
  scanExecution: {
    all: ["scanExecution"] as const,
    detail: (runId: number) => ["scanExecution", "detail", runId] as const,
    history: (projectId: number | null, envUrl: string | null, limit: number) =>
      ["scanExecution", "history", projectId, envUrl, limit] as const,
  },
  /** Derived code-scan audit for a checked-out project path (`run_code_scan_audit`). */
  codeScanAudit: {
    all: ["codeScanAudit"] as const,
    projectScope: (projectId: number) => ["codeScanAudit", projectId] as const,
    forProject: (projectId: number, projectPath: string) =>
      ["codeScanAudit", projectId, projectPath] as const,
  },
  /** Unified work items for a project + environment url (`get_work_items`). */
  workItems: {
    all: ["workItems"] as const,
    projectScope: (projectId: number) => ["workItems", projectId] as const,
    forEnv: (projectId: number, envUrl: string) => ["workItems", projectId, envUrl] as const,
  },
  /** One page's canonical issue groups. */
  pageIssues: {
    all: ["pageIssues"] as const,
    projectScope: (projectId: number) => ["pageIssues", projectId] as const,
    forPage: (projectId: number, envUrl: string, pageUrl: string) =>
      ["pageIssues", projectId, envUrl, pageUrl] as const,
  },
  /** Page rollups used by the Issues > By Page index. */
  issuePages: {
    all: ["issuePages"] as const,
    projectScope: (projectId: number) => ["issuePages", projectId] as const,
    forEnv: (projectId: number, envUrl: string) => ["issuePages", projectId, envUrl] as const,
  },
  /** Dossier lifecycle memory, including environment labels and deploy context. */
  issueMemory: {
    all: ["issueMemory"] as const,
    projectScope: (projectId: number) => ["issueMemory", projectId] as const,
    forCheck: (projectId: number, checkId: string, status: string | null) =>
      ["issueMemory", projectId, checkId, status] as const,
  },
  /** Existing external tracker link for one canonical issue. */
  issueLinks: {
    all: ["issueLinks"] as const,
    projectScope: (projectId: number) => ["issueLinks", projectId] as const,
    forCheck: (projectId: number, checkId: string) => ["issueLinks", projectId, checkId] as const,
  },
  /** Resolved issue history for one project/environment. */
  resolvedIssues: {
    all: ["resolvedIssues"] as const,
    projectScope: (projectId: number) => ["resolvedIssues", projectId] as const,
    forEnv: (projectId: number, envUrl: string, limit: number) =>
      ["resolvedIssues", projectId, envUrl, limit] as const,
  },
  /** Alert rows and unread counts. Counts are shared by page, dashboard, and nav. */
  alerts: {
    all: ["alerts"] as const,
    projectScope: (projectId: number) => ["alerts", projectId] as const,
    rows: (projectId: number, filter: string) => ["alerts", projectId, "rows", filter] as const,
    counts: (projectId: number) => ["alerts", projectId, "counts"] as const,
    /** Connected alerts, scoped by project invalidation and environment binding. */
    connected: (projectId: number, environmentScopeKey: string) =>
      ["alerts", projectId, "connected", environmentScopeKey] as const,
  },
  /** Event timeline ranges and update-only history. */
  events: {
    all: ["events"] as const,
    projectScope: (projectId: number) => ["events", projectId] as const,
    range: (projectId: number, startDate: string, endDate: string, eventTypes: string) =>
      ["events", projectId, "range", startDate, endDate, eventTypes] as const,
    updates: (projectId: number) => ["events", projectId, "updates"] as const,
    dashboardRecent: (projectId: number) => ["events", projectId, "dashboardRecent"] as const,
    dashboardUpdates: (projectId: number) => ["events", projectId, "dashboardUpdates"] as const,
  },
  /** Connected analytics keys, including a project-wide invalidation prefix. */
  analytics: {
    all: ["analytics"] as const,
    projectScope: (projectId: number) => ["analytics", projectId] as const,
    forProject: (projectId: number) => ["analytics", projectId] as const,
    forQuery: (projectId: number, period: string, siteUrl: string | null) =>
      ["analytics", projectId, period, siteUrl] as const,
  },
  /** Configured external integrations for one project. */
  integrations: {
    all: ["integrations"] as const,
    projectScope: (projectId: number) => ["integrations", projectId] as const,
    forProject: (projectId: number) => ["integrations", projectId] as const,
    data: (projectId: number, integrationType: string, urlFilter: string) =>
      ["integrations", projectId, "data", integrationType, urlFilter] as const,
  },
  /** Deployment-page source data. */
  deploys: {
    all: ["deploys"] as const,
    projectScope: (projectId: number) => ["deploys", projectId] as const,
    overview: (projectId: number, envUrl: string, projectPath: string | null) =>
      ["deploys", projectId, envUrl, projectPath, "overview"] as const,
    github: (projectId: number) => ["deploys", projectId, "github"] as const,
  },
  /** Latest Web Scan SEO evidence shown on Search & SEO. */
  searchScan: {
    all: ["searchScan"] as const,
    forProject: (projectId: number, envUrl: string) => ["searchScan", projectId, envUrl] as const,
  },
  /** Dependency update report and its durable timeline. */
  updates: {
    all: ["updates"] as const,
    projectScope: (projectId: number) => ["updates", projectId] as const,
    report: (projectId: number, projectPath: string, envUrl: string) =>
      ["updates", projectId, projectPath, envUrl, "report"] as const,
  },
  /** Report-builder source snapshots and saved report history. */
  reports: {
    all: ["reports"] as const,
    projectScope: (projectId: number) => ["reports", projectId] as const,
    snapshot: (projectId: number, siteUrl: string, periodDays: number, sectionsKey: string) =>
      ["reports", projectId, siteUrl, periodDays, sectionsKey, "snapshot"] as const,
    history: (projectId: number) => ["reports", projectId, "history"] as const,
  },
  /** Multi-project overview payload. */
  sites: {
    all: ["sites"] as const,
    overview: () => ["sites", "overview"] as const,
  },
  /** Refetchable settings reads; durable preferences remain in their domain stores. */
  settings: {
    all: ["settings"] as const,
    connectedStatus: (projectId: number, environmentScopeKey: string) =>
      ["settings", "connectedStatus", projectId, environmentScopeKey] as const,
    /** Service-authoritative connected-site state. */
    connectedRemoteState: (projectId: number, environmentScopeKey: string) =>
      ["settings", "connectedRemoteState", projectId, environmentScopeKey] as const,
    /** The connected site's report registry (`list_connected_reports`). */
    connectedReports: (projectId: number, environmentScopeKey: string) =>
      ["settings", "connectedReports", projectId, environmentScopeKey] as const,
    /** The connected site's outbound alert webhooks (`list_connected_alert_webhooks`). */
    connectedAlertWebhooks: (projectId: number, environmentScopeKey: string) =>
      ["settings", "connectedAlertWebhooks", projectId, environmentScopeKey] as const,
    /** The account's alert email destinations (`list_connected_destinations`);
     *  account-level, so no project or environment in the key. */
    connectedDestinations: () => ["settings", "connectedDestinations"] as const,
    /** One site's alert routing (`get_connected_notification_settings`). */
    connectedNotificationSettings: (projectId: number, environmentScopeKey: string) =>
      ["settings", "connectedNotificationSettings", projectId, environmentScopeKey] as const,
    /** The connected site's machine credentials (`list_connected_site_credentials`). */
    connectedSiteCredentials: (projectId: number, environmentScopeKey: string) =>
      ["settings", "connectedSiteCredentials", projectId, environmentScopeKey] as const,
    /** The account's provider connections (`list_connected_provider_connections`);
     *  account-level, so no project or environment in the key. */
    connectedProviderConnections: () => ["settings", "connectedProviderConnections"] as const,
    /** One connection's provider-side projects (`list_connected_provider_projects`). */
    connectedProviderProjects: (connectionId: string) =>
      ["settings", "connectedProviderProjects", connectionId] as const,
    /** The account's pending admin recovery (`get_account_recovery`). */
    accountRecovery: () => ["settings", "accountRecovery"] as const,
    webhooks: (projectId: number) => ["settings", "webhooks", projectId] as const,
    scanSchedule: (projectId: number, environmentId: number) =>
      ["settings", "scanSchedule", projectId, environmentId] as const,
    sitemapSite: (siteUrl: string, projectId?: number) =>
      ["settings", "sitemapSite", projectId ?? null, siteUrl] as const,
    sitemapPages: (siteId: number) => ["settings", "sitemapPages", siteId] as const,
    databaseInfo: () => ["settings", "databaseInfo"] as const,
    pagespeedKey: () => ["settings", "pagespeedKey"] as const,
    agentTools: () => ["settings", "agentTools"] as const,
    autostart: () => ["settings", "autostart"] as const,
    appVersion: () => ["settings", "appVersion"] as const,
    catalogStatus: () => ["settings", "catalogStatus"] as const,
  },
  /** The site's verified-good baseline per fact family (`get_site_baseline`). */
  siteBaseline: {
    all: ["siteBaseline"] as const,
    forScope: (siteId: number, projectId?: number, environmentScopeKey?: string) =>
      ["siteBaseline", siteId, projectId ?? null, environmentScopeKey ?? null] as const,
  },
  /** TLS certificate probe for an environment url (`check_ssl`). */
  sslProbe: {
    all: ["sslProbe"] as const,
    forUrl: (envUrl: string) => ["sslProbe", envUrl] as const,
  },
  /** Dashboard caches grouped beneath a normalized project/environment key. */
  projectSummary: {
    all: ["projectSummary"] as const,
    projectScope: (projectId: number) => ["projectSummary", projectId] as const,
    forProject: (projectId: number, envUrl: string) =>
      ["projectSummary", projectId, envUrl] as const,
    snapshot: (projectId: number, envUrl: string) =>
      ["projectSummary", projectId, envUrl, "snapshot"] as const,
    navBadge: (projectId: number, envUrl: string) =>
      ["projectSummary", projectId, envUrl, "navBadge"] as const,
    referenceSignals: (projectId: number, envUrl: string, includePsi: boolean) =>
      ["projectSummary", projectId, envUrl, "referenceSignals", includePsi] as const,
    signals: (projectId: number, envUrl: string) =>
      ["projectSummary", projectId, envUrl, "signals"] as const,
  },
} as const;
