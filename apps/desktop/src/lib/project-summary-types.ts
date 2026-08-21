import type { AppTarget } from "@/lib/app-targets";
import type {
  CheckResult,
  CodeIssue,
  CodeScanDomain,
  CodeScanResult,
  CodeScanSummary,
  IssueLink,
  IssueStatus,
  ScanResult,
  UpdateReport,
} from "@/lib/types";

export interface LatestCodeScanSnapshot {
  summary: CodeScanSummary | null;
  result: CodeScanResult | null;
}

export interface SearchRegressionSignal {
  source: string;
  deltaPct: number;
  focus?: string | null;
  itemId?: string | null;
}

interface ProjectMonitoringSignals {
  enabledIntegrations: string[];
  integrationFailureCount: number;
  staleIntegrationCount: number;
  searchRegression: SearchRegressionSignal | null;
}

interface ProjectAttentionTargets {
  securityIssueId: string | null;
  securityFocus: string | null;
}

type WorkItemKind = "web" | "code" | "launch" | "update";

// Project summaries add `working` to the persisted issue lifecycle vocabulary.
export type WorkItemStatus = IssueStatus | "working";

export interface ProjectWorkItem {
  stableKey: string;
  projectId: number;
  environmentUrl: string | null;
  kind: WorkItemKind;
  status: WorkItemStatus;
  severity: string | null;
  title: string;
  summary: string;
  category: string | null;
  domain: string | null;
  packageName: string | null;
  target: AppTarget;
  firstSeenAt: string;
  lastSeenAt: string;
  lastVerifiedAt: string | null;
  lastStatusChangedAt: string;
}

export interface ProjectWorkQueue {
  resumeNow: ProjectWorkItem[];
  verifyNow: ProjectWorkItem[];
  fixNext: ProjectWorkItem[];
  maintenance: ProjectWorkItem[];
}

export interface ProjectWorkSummary {
  /** Active group counts; optional only for pre-cutover cached snapshots. */
  issueCount?: number;
  issueWebCount?: number;
  issueCodeCount?: number;
  issueCriticalCount?: number;
  issueHighCount?: number;
  issueMediumCount?: number;
  issueLowCount?: number;
  unresolvedCount: number;
  newCount: number;
  workingCount: number;
  regressedCount: number;
  ignoredCount: number;
  blockedCount: number;
  launchBlockerCount: number;
  maintenanceCount: number;
  primaryAction: ProjectWorkItem | null;
  regressedAction: ProjectWorkItem | null;
  workingAction: ProjectWorkItem | null;
  blockedAction: ProjectWorkItem | null;
  ignoredAction: ProjectWorkItem | null;
  launchBlockerAction: ProjectWorkItem | null;
  weeklySummary: ProjectWorkItem | null;
}

export interface ProjectSignalSnapshot {
  projectId: number;
  environmentUrl: string | null;
  firstScanBannerDismissed: boolean;
  codeScanSummary: CodeScanSummary | null;
  previousCodeScanSummary: CodeScanSummary | null;
  codeScanDetail: CodeScanResult | null;
  monitoring: ProjectMonitoringSignals;
  monitoringRefreshedAt: string | null;
  updates: UpdateReport | null;
  updatesRefreshedAt: string | null;
  targets: ProjectAttentionTargets;
  workSummary: ProjectWorkSummary;
}

interface DashboardAggregatedCheckCounts {
  passed: number;
  total: number;
  failed: number;
}

export interface DashboardWorkflowRun {
  name: string;
  conclusion: string | null;
  status: string;
  htmlUrl: string;
  updatedAt: string;
}

export interface DashboardPagespeedReport {
  performanceScore: number;
  lcpMs: number | null;
  cls: number | null;
  tbtMs: number | null;
  fcpMs: number | null;
}

interface DashboardIntegrationData {
  integrationType: string;
  data: unknown;
  fetchedAt: string;
  error: string | null;
}

export interface DashboardReferenceSignals {
  integrations: DashboardIntegrationData[];
  lastCiRun: DashboardWorkflowRun | null;
  psiReport: DashboardPagespeedReport | null;
}

export interface DashboardCodeTrendPoint {
  score: number;
  timestamp: string;
  issueCount?: number;
  criticalCount?: number;
  highCount?: number;
}

export interface DashboardSnapshot {
  projectId: number;
  environmentUrl: string | null;
  trend: Array<{
    overall: number;
    security: number | null;
    performance: number | null;
    seo: number | null;
    accessibility: number | null;
    compliance: number | null;
    config: number | null;
    polish?: number | null;
    timestamp: string;
    issues: number;
    scanType: string;
  }>;
  codeTrend: DashboardCodeTrendPoint[];
  latestScanId: number | null;
  latestDetail: ScanResult | null;
  previousDetail: ScanResult | null;
  aggregatedCheckCounts: DashboardAggregatedCheckCounts;
  aggregatedFailedIssues: CheckResult[];
  commitsSinceLastScan: Array<{
    hash: string;
    shortHash: string;
    message: string;
    author: string;
    date: string;
    relativeDate: string;
  }>;
  issueLinks: IssueLink[];
  inactiveCheckIds: string[];
  signals: ProjectSignalSnapshot;
  workQueue: ProjectWorkQueue;
}

export interface ProjectNavBadgeSnapshot {
  projectId: number;
  environmentUrl: string | null;
  aggregatedFailedIssues: CheckResult[];
  inactiveCheckIds: string[];
  signals: ProjectSignalSnapshot;
}

export interface TodayProjectWorkSummary {
  id: number;
  name: string;
  framework: string | null;
  primaryUrl: string;
  latestScore: number | null;
  siteScore: number | null;
  siteIssueCount: number;
  siteCriticalCount: number;
  siteHighCount: number;
  lastScannedAt: string | null;
  issuesCritical: number;
  issuesHigh: number;
  environmentCount: number;
  projectPath: string | null;
  primarySecurityIssueId: string | null;
  primarySecurityFocus: string | null;
  enabledIntegrations: string[];
  securityUpdateCount: number;
  pendingUpdateCount: number;
  searchRegression: SearchRegressionSignal | null;
  integrationFailureCount: number;
  staleIntegrationCount: number;
  guardrailCriticalCount: number;
  guardrailHighCount: number;
  topGuardrailIssue: CodeIssue | null;
  topGuardrailDomain: CodeScanDomain | null;
  topGuardrailDomainCount: number;
  guardrailsCheckedAt: string | null;
  codeScanCheckedAt: string | null;
  workSummary: ProjectWorkSummary;
}
