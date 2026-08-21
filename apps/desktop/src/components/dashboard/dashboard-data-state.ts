import type {
  CodeScanResult,
  CodeScanSummary,
  IssueLink,
  PackageUpdate,
  ScanResult,
} from "@/lib/types";
import type {
  DashboardPagespeedReport,
  DashboardReferenceSignals,
  DashboardCodeTrendPoint,
  DashboardSnapshot,
  DashboardWorkflowRun,
  ProjectWorkQueue,
  ProjectWorkSummary,
  SearchRegressionSignal,
} from "@/lib/project-summary-signals";
import { EMPTY_PROJECT_WORK_SUMMARY } from "@/lib/project-work-summary";
import { buildUpdateQueueSummary } from "@/lib/update-summary";
import type { ScoreTrendPoint } from "./DashboardComponents";

export interface IntegrationData {
  integrationType: string;
  data: unknown;
  error: string | null;
}

type CommitSummary = {
  hash: string;
  short_hash: string;
  message: string;
  author: string;
  timestamp: string;
};

interface DashboardDataState {
  trend: ScoreTrendPoint[];
  codeTrend: DashboardCodeTrendPoint[];
  latestDetail: ScanResult | null;
  previousDetail: ScanResult | null;
  latestScanId: number | null;
  securityUpdates: PackageUpdate[];
  allUpdates: PackageUpdate[];
  integrations: IntegrationData[];
  configuredIntegrations: Set<string>;
  lastCIRun: DashboardWorkflowRun | null;
  commitsSinceLastScan: CommitSummary[];
  issueLinks: IssueLink[];
  aggregatedCheckCounts: {
    passed: number;
    total: number;
    failed: number;
  };
  aggregatedFailedIssues: ScanResult["issues"];
  psiReport: DashboardPagespeedReport | null;
  dashboardLoadError: string | null;
  // Distinguishes a hydrated empty snapshot from an unhydrated reset state.
  snapshotHydrated: boolean;
  dismissedIds: Set<string>;
  dismissedProjectId: number;
  latestCodeScanSummary: CodeScanSummary | null;
  previousCodeScanSummary: CodeScanSummary | null;
  latestCodeScanDetail: CodeScanResult | null;
  updatesCheckedAt: string | null;
  searchRegression: SearchRegressionSignal | null;
  integrationFailureCount: number;
  staleIntegrationCount: number;
  firstScanBannerDismissed: boolean;
  workQueue: ProjectWorkQueue;
  workSummary: ProjectWorkSummary;
  referenceSignalsLoading: boolean;
}

type DashboardDataAction =
  | {
      type: "reset";
      projectId: number;
      referenceSignalsLoading?: boolean;
    }
  | {
      type: "snapshotLoaded";
      snapshot: DashboardSnapshot;
      projectId: number;
      fallbackUpdates?: PackageUpdate[] | null;
    }
  | { type: "snapshotFailed"; message: string }
  | { type: "referenceSignalsStarted" }
  | {
      type: "referenceSignalsLoaded";
      signals: DashboardReferenceSignals;
      includePsi: boolean;
    }
  | { type: "referenceSignalsFailed"; includePsi: boolean }
  | { type: "dismissFirstScanBanner" }
  | { type: "restoreFirstScanBanner" };

const EMPTY_CHECK_COUNTS = { passed: 0, total: 0, failed: 0 };
const EMPTY_WORK_QUEUE: ProjectWorkQueue = {
  resumeNow: [],
  verifyNow: [],
  fixNext: [],
  maintenance: [],
};
function mapCommits(snapshot: DashboardSnapshot): CommitSummary[] {
  return snapshot.commitsSinceLastScan.map((commit) => ({
    hash: commit.hash,
    short_hash: commit.shortHash,
    message: commit.message,
    author: commit.author,
    timestamp: commit.date,
  }));
}

function createEmptyDashboardDataState(
  projectId: number,
  options?: { referenceSignalsLoading?: boolean },
): DashboardDataState {
  return {
    trend: [],
    codeTrend: [],
    latestDetail: null,
    previousDetail: null,
    latestScanId: null,
    securityUpdates: [],
    allUpdates: [],
    integrations: [],
    configuredIntegrations: new Set(),
    lastCIRun: null,
    commitsSinceLastScan: [],
    issueLinks: [],
    aggregatedCheckCounts: EMPTY_CHECK_COUNTS,
    aggregatedFailedIssues: [],
    psiReport: null,
    dashboardLoadError: null,
    snapshotHydrated: false,
    dismissedIds: new Set(),
    dismissedProjectId: projectId,
    latestCodeScanSummary: null,
    previousCodeScanSummary: null,
    latestCodeScanDetail: null,
    updatesCheckedAt: null,
    searchRegression: null,
    integrationFailureCount: 0,
    staleIntegrationCount: 0,
    firstScanBannerDismissed: false,
    workQueue: EMPTY_WORK_QUEUE,
    workSummary: EMPTY_PROJECT_WORK_SUMMARY,
    referenceSignalsLoading: options?.referenceSignalsLoading ?? false,
  };
}

export function createDashboardDataStateFromSnapshot(
  snapshot: DashboardSnapshot | null | undefined,
  projectId: number,
  referenceSignals?: DashboardReferenceSignals | null,
  fallbackUpdates?: PackageUpdate[] | null,
): DashboardDataState {
  const referenceSignalState = referenceSignals
    ? {
        integrations: referenceSignals.integrations as IntegrationData[],
        lastCIRun: referenceSignals.lastCiRun,
        psiReport: referenceSignals.psiReport,
      }
    : null;

  if (!snapshot) {
    return {
      ...createEmptyDashboardDataState(projectId),
      ...(referenceSignalState ?? {}),
    };
  }

  const updates = snapshot.signals.updates
    ? snapshot.signals.updates.updates
    : (fallbackUpdates ?? []);
  const updateSummary = buildUpdateQueueSummary(updates);
  return {
    ...createEmptyDashboardDataState(projectId),
    ...(referenceSignalState ?? {}),
    snapshotHydrated: true,
    trend: snapshot.trend as ScoreTrendPoint[],
    codeTrend: snapshot.codeTrend ?? [],
    latestDetail: snapshot.latestDetail,
    previousDetail: snapshot.previousDetail,
    latestScanId: snapshot.latestScanId,
    securityUpdates: updateSummary.securityUpdates,
    allUpdates: updates,
    configuredIntegrations: new Set(snapshot.signals.monitoring.enabledIntegrations),
    commitsSinceLastScan: mapCommits(snapshot),
    issueLinks: snapshot.issueLinks,
    aggregatedCheckCounts: snapshot.aggregatedCheckCounts,
    aggregatedFailedIssues: snapshot.aggregatedFailedIssues,
    dismissedIds: new Set(snapshot.inactiveCheckIds),
    dismissedProjectId: projectId,
    latestCodeScanSummary: snapshot.signals.codeScanSummary,
    previousCodeScanSummary: snapshot.signals.previousCodeScanSummary,
    latestCodeScanDetail: snapshot.signals.codeScanDetail,
    updatesCheckedAt: snapshot.signals.updatesRefreshedAt,
    searchRegression: snapshot.signals.monitoring.searchRegression,
    integrationFailureCount: snapshot.signals.monitoring.integrationFailureCount,
    staleIntegrationCount: snapshot.signals.monitoring.staleIntegrationCount,
    firstScanBannerDismissed: snapshot.signals.firstScanBannerDismissed,
    workQueue: snapshot.workQueue,
    workSummary: snapshot.signals.workSummary,
  };
}

export function dashboardDataReducer(
  state: DashboardDataState,
  action: DashboardDataAction,
): DashboardDataState {
  switch (action.type) {
    case "reset":
      return createEmptyDashboardDataState(action.projectId, {
        referenceSignalsLoading: action.referenceSignalsLoading,
      });
    case "snapshotLoaded":
      return {
        ...state,
        ...createDashboardDataStateFromSnapshot(
          action.snapshot,
          action.projectId,
          undefined,
          action.fallbackUpdates,
        ),
        integrations: state.integrations,
        lastCIRun: state.lastCIRun,
        psiReport: state.psiReport,
        referenceSignalsLoading: state.referenceSignalsLoading,
      };
    case "snapshotFailed":
      return { ...state, dashboardLoadError: action.message };
    case "referenceSignalsStarted":
      return { ...state, referenceSignalsLoading: true };
    case "referenceSignalsLoaded":
      return {
        ...state,
        integrations: action.signals.integrations as IntegrationData[],
        lastCIRun: action.signals.lastCiRun,
        psiReport: action.includePsi ? action.signals.psiReport : state.psiReport,
        referenceSignalsLoading: false,
      };
    case "referenceSignalsFailed":
      return {
        ...state,
        integrations: [],
        lastCIRun: null,
        psiReport: action.includePsi ? null : state.psiReport,
        referenceSignalsLoading: false,
      };
    case "dismissFirstScanBanner":
      return { ...state, firstScanBannerDismissed: true };
    case "restoreFirstScanBanner":
      return { ...state, firstScanBannerDismissed: false };
    default:
      return state;
  }
}
