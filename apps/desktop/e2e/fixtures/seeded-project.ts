/** One typed project fixture with a completed scan and one open web issue. */
import type {
  AgentToolStatus,
  CheckResult,
  DashboardReferenceSignals,
  DashboardSnapshot,
  EnvironmentRecord,
  IntegrationConfig,
  IssueGroup,
  IssueInstance,
  ProjectNavBadgeSnapshot,
  NormalizedRunDiagnostics,
  ProjectRecord,
  ProjectSignalSnapshot,
  ProjectWorkSummary,
  RunScanExecutionResult,
  ScanExecutionRecord,
  ScanExecutionSummary,
  ScanResult,
  ScanRunSummary,
  ScanSummary,
  ScoreSnapshot,
  SiteBaseline,
  ScoreTrendPoint,
} from "../../src/generated/ipc-bindings";
import type { InvokeResponses } from "./tauri-stub";

const SCAN_TIMESTAMP = "2026-04-15T12:00:00Z";
const SEEDED_URL = "https://example.com";
export const SEEDED_SCORE = 71;
export const SEEDED_ISSUE_TITLE = "Missing HSTS header";

const environment: EnvironmentRecord = {
  id: 10,
  url: SEEDED_URL,
  label: "Production",
  environment: "production",
  source: null,
  lastScannedAt: SCAN_TIMESTAMP,
  latestScore: SEEDED_SCORE,
};

// An empty path models a URL-only project.
export const seededProject: ProjectRecord = {
  id: 1,
  name: "Example",
  path: "",
  framework: null,
  createdAt: "2026-04-01T09:00:00Z",
  environments: [environment],
};

const seededIssue: CheckResult = {
  checkId: "security.headers.hsts",
  category: "security",
  title: SEEDED_ISSUE_TITLE,
  description: "Strict-Transport-Security is not set, so browsers may downgrade to HTTP.",
  status: "fail",
  severity: "high",
  fixPrompt: null,
  manualFix: null,
  rawData: null,
  confidence: "high",
};

const seededScanSummary: ScanSummary = {
  id: 100,
  url: SEEDED_URL,
  mode: "live",
  scanType: "health",
  overallScore: SEEDED_SCORE,
  issuesTotal: 1,
  issuesCritical: 0,
  issuesHigh: 1,
  issuesMedium: 0,
  issuesLow: 0,
  durationMs: 900,
  timestamp: SCAN_TIMESTAMP,
  sessionId: null,
  pageUrl: null,
};

export const seededDetail: ScanResult = {
  url: SEEDED_URL,
  mode: "live",
  scanType: "health",
  overallScore: SEEDED_SCORE,
  categories: [],
  issues: [seededIssue],
  detectedStack: null,
  durationMs: 900,
  timestamp: SCAN_TIMESTAMP,
};

// Keep the execution id aligned with the dashboard's latest scan id.
const runDiagnostics: NormalizedRunDiagnostics = {
  mode: "live",
  focus: "health",
  securityScore: 62,
  performanceScore: null,
  seoScore: null,
  accessibilityScore: null,
  complianceScore: null,
  configScore: null,
  polishScore: null,
  detectedStack: null,
  pageUrl: null,
  projectPath: null,
  framework: null,
  codeCommitSha: null,
  codeTreeClean: null,
  totalPages: 1,
  completedPages: 1,
  axeEnabled: false,
  browserRan: false,
  browserBuild: null,
};

const seededRunSummary: ScanRunSummary = {
  id: seededScanSummary.id,
  parentRunId: null,
  source: "web_scan",
  runKind: "single",
  status: "complete",
  timestamp: SCAN_TIMESTAMP,
  rawScore: SEEDED_SCORE,
  durationMs: 900,
  issuesTotal: 1,
  issuesCritical: 0,
  issuesHigh: 1,
  issuesMedium: 0,
  issuesLow: 0,
  diagnostics: runDiagnostics,
};

const seededExecutionRecord: ScanExecutionRecord = {
  id: 500,
  projectId: seededProject.id,
  environmentId: environment.id,
  environmentUrl: SEEDED_URL,
  environmentScopeKey: SEEDED_URL,
  requestedMode: "web",
  webFocus: "health",
  trigger: "manual",
  admissionClass: "general_scan",
  status: "complete",
  idempotencyKey: "e2e-seeded-scan",
  requestFingerprint: "e2e-seeded-scan",
  startedAt: Date.parse(SCAN_TIMESTAMP),
  completedAt: Date.parse(SCAN_TIMESTAMP) + 900,
  scoreSnapshotId: null,
  failureSummary: null,
  webStatus: "complete",
  webDetail: null,
  codeStatus: null,
  codeDetail: null,
};

/** Completed single-Web execution fixture carrying the summary's `webResult`. */
export function runScanExecutionResult(webResult: ScanResult): RunScanExecutionResult {
  return {
    execution: seededExecutionRecord,
    reused: false,
    webResult,
    multiResult: null,
    codeResult: null,
  };
}

const seededExecutionSummary: ScanExecutionSummary = {
  id: 500,
  projectId: seededProject.id,
  environmentId: environment.id,
  environmentUrl: SEEDED_URL,
  requestedMode: "web",
  webFocus: "health",
  trigger: "manual",
  status: "complete",
  startedAt: Date.parse(SCAN_TIMESTAMP),
  completedAt: Date.parse(SCAN_TIMESTAMP) + 900,
  score: SEEDED_SCORE,
  criticalCount: 0,
  highCount: 1,
  mediumCount: 0,
  lowCount: 0,
  webStatus: "complete",
  webDetail: null,
  codeStatus: null,
  codeDetail: null,
  webScanId: seededScanSummary.id,
  webSessionId: null,
  webPageCount: 1,
  codeScanId: null,
  runs: [seededRunSummary],
};

// The active web group needs an instance so issue ranking does not classify it as code-only.
const seededIssueInstance: IssueInstance = {
  id: 1,
  source: "web_scan",
  signalId: seededIssue.checkId,
  producerCheckId: seededIssue.checkId,
  url: SEEDED_URL,
  pageUrl: null,
  severity: "high",
  title: SEEDED_ISSUE_TITLE,
  description: seededIssue.description,
  detailJson: null,
  firstSeenAt: Date.parse(SCAN_TIMESTAMP),
  lastSeenAt: Date.parse(SCAN_TIMESTAMP),
  confidence: "high",
  domain: null,
  relativePath: null,
  line: null,
};

const seededWorkItemGroup: IssueGroup = {
  checkId: seededIssue.checkId,
  category: seededIssue.category,
  severity: "high",
  title: SEEDED_ISSUE_TITLE,
  description: seededIssue.description,
  instances: [seededIssueInstance],
  sources: ["web_scan"],
  status: "new",
  snoozeUntil: null,
  blockReason: null,
  impactScore: 9,
  likelyCauses: [],
  suggestedIntegrations: [],
  fixLocations: [],
  transitiveCauses: [],
  downstreamEffects: [],
  recentEvents: [],
  enrichments: [],
  correlationEvidence: [],
  affectedPages: [SEEDED_URL],
  crossEnvSignal: null,
  crossProjectPattern: null,
  displayConfidence: "high",
  observationCount: 1,
  anomalyScore: null,
};

const trendPoint: ScoreTrendPoint = {
  overall: SEEDED_SCORE,
  security: 62,
  performance: null,
  seo: null,
  accessibility: null,
  compliance: null,
  config: null,
  polish: null,
  timestamp: SCAN_TIMESTAMP,
  issues: 1,
  scanType: "health",
};

const emptyWorkSummary: ProjectWorkSummary = {
  issueCount: 1,
  issueWebCount: 1,
  issueCodeCount: 0,
  issueCriticalCount: 0,
  issueHighCount: 1,
  issueMediumCount: 0,
  issueLowCount: 0,
  unresolvedCount: 1,
  newCount: 1,
  workingCount: 0,
  regressedCount: 0,
  ignoredCount: 0,
  blockedCount: 0,
  launchBlockerCount: 0,
  maintenanceCount: 0,
  primaryAction: null,
  regressedAction: null,
  workingAction: null,
  blockedAction: null,
  ignoredAction: null,
  launchBlockerAction: null,
  weeklySummary: null,
};

const signals: ProjectSignalSnapshot = {
  projectId: seededProject.id,
  environmentUrl: SEEDED_URL,
  firstScanBannerDismissed: true,
  codeScanSummary: null,
  previousCodeScanSummary: null,
  codeScanDetail: null,
  monitoring: {
    enabledIntegrations: [],
    integrationFailureCount: 0,
    staleIntegrationCount: 0,
    searchRegression: null,
  },
  monitoringRefreshedAt: null,
  updates: null,
  updatesRefreshedAt: null,
  targets: {
    securityIssueId: null,
    securityFocus: null,
  },
  workSummary: emptyWorkSummary,
};

const seededSnapshot: DashboardSnapshot = {
  projectId: seededProject.id,
  environmentUrl: SEEDED_URL,
  trend: [trendPoint],
  codeTrend: [],
  latestScanId: seededScanSummary.id,
  latestDetail: seededDetail,
  previousDetail: null,
  aggregatedCheckCounts: { passed: 9, total: 10, failed: 1 },
  aggregatedFailedIssues: [seededIssue],
  commitsSinceLastScan: [],
  issueLinks: [],
  inactiveCheckIds: [],
  signals,
  workQueue: { resumeNow: [], verifyNow: [], fixNext: [], maintenance: [] },
};

const seededScore: ScoreSnapshot = {
  overall: SEEDED_SCORE,
  perCategory: { security: 62 },
  criticalCount: 0,
  highCount: 1,
  mediumCount: 0,
  lowCount: 0,
  exploitableCapped: false,
  breakdown: {
    base: 100,
    criticalPoints: 0,
    highPoints: 9,
    mediumPoints: 0,
    lowPoints: 0,
    effCritical: 0,
    effHigh: 1,
    effMedium: 0,
    effLow: 0,
    floorApplied: false,
  },
  computedAt: Date.parse(SCAN_TIMESTAMP),
};

const referenceSignals: DashboardReferenceSignals = {
  integrations: [],
  lastCiRun: null,
  psiReport: null,
};

const navBadgeSnapshot: ProjectNavBadgeSnapshot = {
  projectId: seededProject.id,
  environmentUrl: SEEDED_URL,
  aggregatedFailedIssues: [seededIssue],
  inactiveCheckIds: [],
  signals,
};

/** Stub overrides for a booted app with one scanned project. */
export function seededProjectResponses(): InvokeResponses {
  return {
    get_projects: [seededProject],
    // Adapters select the matching run kind from this shared execution fixture.
    get_scan_executions: [seededExecutionSummary],
    get_scan_execution_detail: null,
    get_dashboard_snapshot: seededSnapshot,
    get_dashboard_reference_signals: referenceSignals,
    get_current_score: seededScore,
    get_project_nav_badge_snapshot: navBadgeSnapshot,
    get_project_signal_snapshot: signals,
    get_events: [],
    backfill_events: 0,
    get_integrations: [] satisfies IntegrationConfig[],
    detect_agent_tools: [] satisfies AgentToolStatus[],
    get_work_items: [seededWorkItemGroup],
    get_resolved_issues: [],
    get_or_create_site_id: 1,
    get_site_pages: [],
    generate_report_data: null,
    get_report_history: [],
    // Revision zero represents a baseline that has not recorded any fields.
    get_site_baseline: { revision: 0, fields: [] } satisfies SiteBaseline,
    update_tray_summary: null,
    update_tray_scan_status: null,
  };
}
