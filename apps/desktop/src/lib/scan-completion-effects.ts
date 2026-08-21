import {
  getScoreMessage,
  type CodeScanResult,
  type CodeScanSummary,
  type ScanResult,
} from "@/lib/types";
import { CODE_SCAN_FOCUS, normalizeAppUrlForOptionalKey, type AppTarget } from "@/lib/app-targets";
import {
  CODE_SCAN_DOMAIN_META,
  CODE_SCAN_DOMAIN_ORDER,
  getCodeIssueDomain,
  type CodeScanDomain,
} from "@/lib/code-scan-domains";
import { normalizeCodeScanResult } from "@/lib/code-scan-result-normalize";
import { getPreviousCodeScanSummary } from "@/lib/code-scan-comparison";
import {
  buildCodeScanSummaryFromResult,
  describeCodeScanDomainTrend,
} from "@/lib/code-scan-summary-insights";
import {
  getProjectSignalSnapshot,
  primeLatestCodeScanSnapshot,
} from "@/lib/project-summary-signals";
import { getPrimaryWorkSummaryCue } from "@/lib/work-item-presentation";
import {
  buildCodeScanCompletionCopy,
  buildMultiScanCompletionCopy,
  buildWebScanCompletionCopy,
} from "@/lib/scan-completion-copy";
import {
  buildOpenTargetNotificationAction,
  buildScanResultNotificationActions,
} from "@/lib/notification-actions";
import { sendActionableDesktopNotification } from "@/lib/actionable-notifications";
import { removeOnboardingSetupStep } from "@/lib/onboarding-setup";
import { currentScoreIssueCount, loadCurrentScoreSnapshot } from "@/lib/current-score";
import type { ScanSessionSummary } from "@/hooks/useHistory";
import type { PostScanFollowUpBanner, WorkflowCue } from "@/lib/scan-follow-up";
import { getWorkflowNotificationFollowUpAction } from "@/lib/scan-follow-up";
import { SCAN_LABELS } from "@/lib/scan-labels";
import { formatUrlDisplay } from "@/lib/utils";
import { isActionableCheckResult } from "@/lib/issues";

interface ScanJobContext {
  projectId?: number | null;
  url?: string | null;
  scopeLabel?: string | null;
}

interface CompletionRuntime {
  activeScanScope: string;
  desktopNotificationsEnabled: boolean;
  openAppTarget: (target: AppTarget) => void;
  refreshProjects: () => void;
  setScanFollowUpBanner: (banner: PostScanFollowUpBanner | null) => void;
  toast: {
    success: (title: string, body?: string) => void;
    error: (title: string, body?: string) => void;
  };
}

interface CompletionScore {
  score: number;
  scoreMessage: string;
  issueCount: number | null;
  fromSnapshot: boolean;
}

interface CodeScanCompletionParams extends CompletionRuntime {
  codeHistory: CodeScanSummary[];
  codeResult: CodeScanResult;
  activeEnvUrl: string | null | undefined;
  activeProjectId: number | null | undefined;
  currentProjectName: string | null | undefined;
  scanBackgrounded: boolean;
  scanContext: ScanJobContext | null;
  completeJob: (
    kind: "scan",
    payload: {
      label: string;
      scopeLabel: string;
      detail: string;
      target: AppTarget;
    },
  ) => void;
  loadHistory: (url: string, projectId?: number) => Promise<void>;
}

interface WebScanCompletionParams extends CompletionRuntime {
  result: ScanResult;
  history: Array<{ url: string; overallScore: number; issuesTotal: number }>;
  activeEnvUrl: string | null | undefined;
  activeProjectId: number | null | undefined;
  scanBackgrounded: boolean;
  scanContext: ScanJobContext | null;
  completeJob: (
    kind: "scan",
    payload: {
      label: string;
      scopeLabel: string;
      detail: string;
      target: AppTarget;
    },
  ) => void;
  loadHistory: (url: string, projectId?: number) => Promise<void>;
  loadLatestWebScanId: (projectId: number | null, url: string) => Promise<number | null>;
}

interface FullScanCompletionParams extends CompletionRuntime {
  result: ScanResult;
  codeResult: CodeScanResult;
  activeEnvUrl: string | null | undefined;
  activeProjectId: number | null | undefined;
  currentProjectName: string | null | undefined;
  scanBackgrounded: boolean;
  scanContext: ScanJobContext | null;
  completeJob: (
    kind: "scan",
    payload: {
      label: string;
      scopeLabel: string;
      detail: string;
      target: AppTarget;
    },
  ) => void;
  loadHistory: (url: string, projectId?: number) => Promise<void>;
  loadLatestWebScanId: (projectId: number | null, url: string) => Promise<number | null>;
}

interface MultiScanCompletionParams extends CompletionRuntime {
  multiResult: { overallScore: number; completedPages: number };
  activeEnvUrl: string | null | undefined;
  activeProjectId: number | null | undefined;
  scanBackgrounded: boolean;
  scanContext: ScanJobContext | null;
  completeJob: (
    kind: "scan",
    payload: {
      label: string;
      scopeLabel: string;
      detail: string;
      target: AppTarget;
    },
  ) => void;
  loadHistory: (url: string, projectId?: number) => Promise<void>;
  loadLatestSessionSummary: (
    projectId: number | null,
    url: string,
  ) => Promise<ScanSessionSummary | null>;
}

interface FullMultiScanCompletionParams extends CompletionRuntime {
  multiResult: {
    overallScore: number;
    completedPages: number;
    pageResults: Array<{ issuesCount: number }>;
    siteIssues: unknown[];
  };
  codeResult: CodeScanResult;
  activeEnvUrl: string | null | undefined;
  activeProjectId: number | null | undefined;
  currentProjectName: string | null | undefined;
  scanBackgrounded: boolean;
  scanContext: ScanJobContext | null;
  completeJob: (
    kind: "scan",
    payload: {
      label: string;
      scopeLabel: string;
      detail: string;
      target: AppTarget;
    },
  ) => void;
  loadHistory: (url: string, projectId?: number) => Promise<void>;
  loadLatestSessionSummary: (
    projectId: number | null,
    url: string,
  ) => Promise<ScanSessionSummary | null>;
}

function normalizeCompletionUrl(value?: string | null): string | null {
  return normalizeAppUrlForOptionalKey(value);
}

async function loadCompletionScore(
  projectId: number | null | undefined,
  envUrl: string | null | undefined,
  fallbackScore: number,
): Promise<CompletionScore> {
  if (projectId == null) {
    return {
      score: fallbackScore,
      scoreMessage: getScoreMessage(fallbackScore),
      issueCount: null,
      fromSnapshot: false,
    };
  }

  try {
    const snapshot = await loadCurrentScoreSnapshot(projectId, envUrl ?? null);
    const score = Math.round(snapshot.overall);
    return {
      score,
      scoreMessage: getScoreMessage(score),
      issueCount: currentScoreIssueCount(snapshot),
      fromSnapshot: true,
    };
  } catch {
    return {
      score: fallbackScore,
      scoreMessage: getScoreMessage(fallbackScore),
      issueCount: null,
      fromSnapshot: false,
    };
  }
}

function summarizeLeadingCodeScanDomain(
  result: Pick<CodeScanResult, "issues" | "domainSummaries">,
) {
  const counts = new Map<CodeScanDomain, number>();
  for (const summary of result.domainSummaries ?? []) {
    if (summary.issueCount > 0) {
      counts.set(summary.domain, summary.issueCount);
    }
  }

  if (counts.size === 0) {
    for (const issue of result.issues) {
      const domain = getCodeIssueDomain(issue);
      counts.set(domain, (counts.get(domain) ?? 0) + 1);
    }
  }

  if (counts.size === 0) return null;
  const ranked = Array.from(counts.entries()).sort((a, b) => {
    if (b[1] !== a[1]) return b[1] - a[1];
    return CODE_SCAN_DOMAIN_ORDER.indexOf(a[0]) - CODE_SCAN_DOMAIN_ORDER.indexOf(b[0]);
  });
  const [domain, count] = ranked[0] ?? [];
  if (!domain || !count) return null;
  return { domain, count, meta: CODE_SCAN_DOMAIN_META[domain] };
}

export async function loadPrimaryWorkflowCue(
  projectId: number | null | undefined,
  url: string | null | undefined,
): Promise<WorkflowCue | null> {
  if (!projectId) return null;
  try {
    const snapshot = await getProjectSignalSnapshot(projectId, url ?? null, {
      includeCodeScanDetail: false,
    });
    return getPrimaryWorkSummaryCue(snapshot.workSummary);
  } catch {
    return null;
  }
}

export async function handleCodeScanCompletion({
  codeHistory,
  codeResult,
  activeEnvUrl,
  activeProjectId,
  currentProjectName,
  scanBackgrounded,
  scanContext,
  completeJob,
  loadHistory,
  activeScanScope,
  desktopNotificationsEnabled,
  openAppTarget,
  refreshProjects,
  setScanFollowUpBanner,
  toast,
}: CodeScanCompletionParams): Promise<void> {
  const normalizedCodeResult = normalizeCodeScanResult(codeResult);
  const completionProjectId =
    normalizedCodeResult.projectId ?? scanContext?.projectId ?? activeProjectId ?? null;
  const codeScanUrl = normalizedCodeResult.environmentUrl ?? scanContext?.url ?? activeEnvUrl;
  const completionScore = await loadCompletionScore(
    completionProjectId,
    codeScanUrl ?? null,
    normalizedCodeResult.overallScore,
  );
  const leadingDomain = summarizeLeadingCodeScanDomain(normalizedCodeResult);
  const previousCodeSummary = getPreviousCodeScanSummary(normalizedCodeResult, codeHistory);
  // First code scans with issues open the Issues page.
  const isFirstScan = previousCodeSummary == null;
  const hasIssues = normalizedCodeResult.issueCount > 0;
  const domainTrend = describeCodeScanDomainTrend(
    buildCodeScanSummaryFromResult(normalizedCodeResult),
    previousCodeSummary,
  );
  primeLatestCodeScanSnapshot(normalizedCodeResult);
  const workflowCuePromise = loadPrimaryWorkflowCue(normalizedCodeResult.projectId, codeScanUrl);
  const previousCodeScore = previousCodeSummary?.overallScore ?? null;
  const previousCodeIssueCount = previousCodeSummary?.issueCount ?? 0;
  const codeResolvedCount =
    previousCodeIssueCount > 0
      ? Math.max(0, previousCodeIssueCount - normalizedCodeResult.issueCount)
      : null;
  // The toast headline reports the single unified SiteCMD Score (read from the
  // persisted snapshot by loadCompletionScore); there is no separate code score.
  const copy = buildCodeScanCompletionCopy({
    score: completionScore.score,
    issueCount: completionScore.issueCount ?? normalizedCodeResult.issueCount,
    scoreMessage: completionScore.scoreMessage,
    host: formatUrlDisplay(codeScanUrl, "current project"),
    previousScore: completionScore.fromSnapshot ? null : previousCodeScore,
    resolvedCount: codeResolvedCount,
    leadingDomain: leadingDomain
      ? {
          label: leadingDomain.meta.label,
          shortLabel: leadingDomain.meta.shortLabel,
          count: leadingDomain.count,
        }
      : null,
    domainTrendLabel: domainTrend.label,
    workflowCue: null,
  });
  const primaryTarget: AppTarget = {
    page: "issues",
    projectId: completionProjectId,
    url: codeScanUrl,
    scanId: normalizedCodeResult.id,
    scanKind: "code",
  };

  toast.success(copy.title, copy.body);
  completeJob("scan", {
    label: copy.jobLabel,
    scopeLabel:
      scanContext?.scopeLabel || activeScanScope || currentProjectName || "Current project",
    detail: copy.jobDetail,
    target: primaryTarget,
  });

  const workflowCue = await workflowCuePromise;

  if (scanBackgrounded && desktopNotificationsEnabled) {
    await sendActionableDesktopNotification({
      id: `code-scan:${normalizedCodeResult.projectId}:${normalizedCodeResult.id}`,
      title: copy.title,
      body: copy.body,
      clickTarget: primaryTarget,
      actions: buildScanResultNotificationActions({
        primaryTarget,
        secondaryAction:
          getWorkflowNotificationFollowUpAction(workflowCue, primaryTarget) ??
          buildOpenTargetNotificationAction("open-code-issues", {
            page: "issues",
            projectId: completionProjectId,
            url: codeScanUrl,
            focus: CODE_SCAN_FOCUS,
          }),
      }),
    }).catch(() => undefined);
  }

  refreshProjects();
  // Keyed on the project alone: onboarding and history refresh are
  // project-scoped. Requiring a URL would strand code-only projects.
  if (completionProjectId != null) {
    removeOnboardingSetupStep(completionProjectId, "code-scan");
    await loadHistory(codeScanUrl ?? "", completionProjectId);
    if (!scanBackgrounded) {
      setScanFollowUpBanner(null);
      if (isFirstScan && hasIssues) {
        openAppTarget(primaryTarget);
      }
    }
  }
}

export async function handleFullScanCompletion({
  result,
  codeResult,
  activeEnvUrl,
  activeProjectId,
  currentProjectName,
  scanBackgrounded,
  scanContext,
  completeJob,
  loadHistory,
  loadLatestWebScanId,
  activeScanScope,
  desktopNotificationsEnabled,
  refreshProjects,
  setScanFollowUpBanner,
  toast,
}: FullScanCompletionParams): Promise<void> {
  const normalizedCodeResult = normalizeCodeScanResult(codeResult);
  primeLatestCodeScanSnapshot(normalizedCodeResult);

  const projectId =
    normalizedCodeResult.projectId ?? scanContext?.projectId ?? activeProjectId ?? null;
  const completionUrl =
    scanContext?.url ?? result.url ?? normalizedCodeResult.environmentUrl ?? activeEnvUrl ?? null;
  const host = formatUrlDisplay(completionUrl ?? result.url, "current project");
  // Rust scan scores are fallback snapshots, never frontend recomputation.
  const fallbackScore = Math.min(result.overallScore, normalizedCodeResult.overallScore);
  const fallbackIssueCount =
    result.issues.filter(isActionableCheckResult).length + normalizedCodeResult.issues.length;
  const completionScore = await loadCompletionScore(
    projectId,
    completionUrl ?? result.url ?? null,
    fallbackScore,
  );
  const latestScanIdPromise = loadLatestWebScanId(projectId, completionUrl ?? result.url);
  const workflowCuePromise = loadPrimaryWorkflowCue(projectId, completionUrl ?? result.url);
  const copy = buildWebScanCompletionCopy({
    titleLabel: SCAN_LABELS.full,
    jobLabel: "Full scan",
    score: completionScore.score,
    issueCount: completionScore.issueCount ?? fallbackIssueCount,
    scoreMessage: completionScore.scoreMessage,
    host,
    previousScore: null,
    resolvedCount: null,
    workflowCue: null,
  });
  const primaryTarget: AppTarget = {
    page: "issues",
    projectId,
    url: completionUrl ?? result.url,
    scanId: null,
    scanKind: "site",
  };

  toast.success(copy.title, copy.body);
  completeJob("scan", {
    label: copy.jobLabel,
    scopeLabel: scanContext?.scopeLabel || activeScanScope || currentProjectName || host,
    detail: copy.jobDetail,
    target: primaryTarget,
  });

  const [latestScanId, workflowCue] = await Promise.all([latestScanIdPromise, workflowCuePromise]);
  const resolvedPrimaryTarget: AppTarget = {
    ...primaryTarget,
    scanId: latestScanId,
  };

  if (scanBackgrounded && desktopNotificationsEnabled) {
    await sendActionableDesktopNotification({
      id: `full-scan:${projectId ?? "current"}:${result.timestamp}:${normalizedCodeResult.id}`,
      title: copy.title,
      body: copy.body,
      clickTarget: resolvedPrimaryTarget,
      actions: buildScanResultNotificationActions({
        primaryTarget: resolvedPrimaryTarget,
        secondaryAction:
          getWorkflowNotificationFollowUpAction(workflowCue, resolvedPrimaryTarget) ??
          buildOpenTargetNotificationAction("open-dashboard", {
            page: "dashboard",
            projectId,
            url: completionUrl ?? result.url,
          }),
      }),
    }).catch(() => undefined);
  }

  refreshProjects();
  if (projectId != null) {
    removeOnboardingSetupStep(projectId, "code-scan");
  }

  if (completionUrl) {
    await loadHistory(completionUrl, projectId ?? undefined);
    if (!scanBackgrounded) {
      setScanFollowUpBanner(null);
    }
  }
}

export async function handleFullMultiScanCompletion({
  multiResult,
  codeResult,
  activeEnvUrl,
  activeProjectId,
  currentProjectName,
  scanBackgrounded,
  scanContext,
  completeJob,
  loadHistory,
  loadLatestSessionSummary,
  activeScanScope,
  desktopNotificationsEnabled,
  refreshProjects,
  setScanFollowUpBanner,
  toast,
}: FullMultiScanCompletionParams): Promise<void> {
  // A multi-page web result plus code result is still one Full Scan.
  const normalizedCodeResult = normalizeCodeScanResult(codeResult);
  primeLatestCodeScanSnapshot(normalizedCodeResult);

  const projectId =
    normalizedCodeResult.projectId ?? scanContext?.projectId ?? activeProjectId ?? null;
  const completionUrl =
    scanContext?.url ?? normalizedCodeResult.environmentUrl ?? activeEnvUrl ?? null;
  const host = formatUrlDisplay(completionUrl, "current project");
  // The degraded Full Scan fallback combines its two Rust snapshots.
  const webIssueCount =
    multiResult.pageResults.reduce((sum, page) => sum + page.issuesCount, 0) +
    multiResult.siteIssues.length;
  const fallbackScore = Math.min(multiResult.overallScore, normalizedCodeResult.overallScore);
  const fallbackIssueCount = webIssueCount + normalizedCodeResult.issues.length;
  const completionScore = await loadCompletionScore(projectId, completionUrl, fallbackScore);
  const latestSessionPromise = completionUrl
    ? loadLatestSessionSummary(projectId, completionUrl)
    : Promise.resolve<ScanSessionSummary | null>(null);
  const workflowCuePromise = loadPrimaryWorkflowCue(projectId, completionUrl);
  const copy = buildWebScanCompletionCopy({
    titleLabel: SCAN_LABELS.full,
    jobLabel: "Full scan",
    score: completionScore.score,
    issueCount: completionScore.issueCount ?? fallbackIssueCount,
    scoreMessage: completionScore.scoreMessage,
    host,
    previousScore: null,
    resolvedCount: null,
    workflowCue: null,
  });
  const primaryTarget: AppTarget = {
    page: "issues",
    projectId,
    url: completionUrl,
    sessionId: null,
  };

  toast.success(copy.title, copy.body);
  completeJob("scan", {
    label: copy.jobLabel,
    scopeLabel: scanContext?.scopeLabel || activeScanScope || currentProjectName || host,
    detail: copy.jobDetail,
    target: primaryTarget,
  });

  const [latestSession, workflowCue] = await Promise.all([
    latestSessionPromise,
    workflowCuePromise,
  ]);
  const resolvedPrimaryTarget: AppTarget = {
    ...primaryTarget,
    sessionId: latestSession?.sessionId ?? null,
  };

  if (scanBackgrounded && desktopNotificationsEnabled) {
    await sendActionableDesktopNotification({
      id: `full-multi-scan:${completionUrl ?? "current"}:${multiResult.overallScore}:${normalizedCodeResult.id}`,
      title: copy.title,
      body: copy.body,
      clickTarget: resolvedPrimaryTarget,
      actions: buildScanResultNotificationActions({
        primaryTarget: resolvedPrimaryTarget,
        secondaryAction:
          getWorkflowNotificationFollowUpAction(workflowCue, resolvedPrimaryTarget) ??
          buildOpenTargetNotificationAction("open-dashboard", {
            page: "dashboard",
            projectId,
            url: completionUrl,
          }),
      }),
    }).catch(() => undefined);
  }

  refreshProjects();
  if (projectId != null) {
    removeOnboardingSetupStep(projectId, "code-scan");
  }
  if (completionUrl) {
    await loadHistory(completionUrl, projectId ?? undefined);
    if (!scanBackgrounded) {
      setScanFollowUpBanner(null);
    }
  }
}

export async function handleWebScanCompletion({
  result,
  history,
  activeEnvUrl,
  activeProjectId,
  scanBackgrounded,
  scanContext,
  completeJob,
  loadHistory,
  loadLatestWebScanId,
  activeScanScope,
  desktopNotificationsEnabled,
  openAppTarget,
  refreshProjects,
  setScanFollowUpBanner,
  toast,
}: WebScanCompletionParams): Promise<void> {
  const projectId = scanContext?.projectId ?? activeProjectId ?? null;
  const completionUrl = scanContext?.url ?? result.url ?? activeEnvUrl ?? null;
  const completionScore = await loadCompletionScore(projectId, completionUrl, result.overallScore);
  const latestScanIdPromise = loadLatestWebScanId(projectId, completionUrl ?? result.url);
  const workflowCuePromise = loadPrimaryWorkflowCue(projectId, completionUrl ?? result.url);
  const previousScan = history.find(
    (entry) => normalizeCompletionUrl(entry.url) === normalizeCompletionUrl(completionUrl),
  );
  const previousScore = previousScan?.overallScore ?? null;
  // First scans with findings land on Issues; rescans keep the current page.
  const isFirstScan = previousScan == null;
  const hasIssues = result.issues.length > 0;
  const newFailedIds = new Set(
    result.issues.filter(isActionableCheckResult).map((issue) => issue.checkId),
  );
  const previousIssueCount = previousScan?.issuesTotal ?? 0;
  const resolvedCount =
    previousIssueCount > 0 ? Math.max(0, previousIssueCount - newFailedIds.size) : null;
  // The toast headline reports the single unified SiteCMD Score (read from the
  // persisted snapshot by loadCompletionScore); there is no separate web score.
  const copy = buildWebScanCompletionCopy({
    score: completionScore.score,
    issueCount: completionScore.issueCount ?? result.issues.length,
    scoreMessage: completionScore.scoreMessage,
    host: formatUrlDisplay(completionUrl ?? result.url),
    previousScore: completionScore.fromSnapshot ? null : previousScore,
    resolvedCount,
    workflowCue: null,
  });
  const primaryTarget: AppTarget = {
    page: "issues",
    projectId,
    url: completionUrl ?? result.url,
    scanId: null,
    scanKind: "site",
  };

  toast.success(copy.title, copy.body);
  completeJob("scan", {
    label: copy.jobLabel,
    scopeLabel:
      scanContext?.scopeLabel || activeScanScope || formatUrlDisplay(completionUrl ?? result.url),
    detail: copy.jobDetail,
    target: primaryTarget,
  });

  const [latestScanId, workflowCue] = await Promise.all([latestScanIdPromise, workflowCuePromise]);
  const resolvedPrimaryTarget: AppTarget = {
    ...primaryTarget,
    scanId: latestScanId,
  };

  if (scanBackgrounded && desktopNotificationsEnabled) {
    await sendActionableDesktopNotification({
      id: `scan:${completionUrl ?? result.url}:${result.overallScore}:${result.issues.length}`,
      title: copy.title,
      body: copy.body,
      clickTarget: resolvedPrimaryTarget,
      actions: buildScanResultNotificationActions({
        primaryTarget: resolvedPrimaryTarget,
        secondaryAction:
          getWorkflowNotificationFollowUpAction(workflowCue, resolvedPrimaryTarget) ??
          buildOpenTargetNotificationAction("open-dashboard", {
            page: "dashboard",
            projectId,
            url: completionUrl ?? result.url,
          }),
      }),
    }).catch(() => undefined);
  }

  refreshProjects();
  if (completionUrl) {
    await loadHistory(completionUrl, projectId ?? undefined);
    if (!scanBackgrounded) {
      setScanFollowUpBanner(null);
      if (isFirstScan && hasIssues) {
        openAppTarget(resolvedPrimaryTarget);
      }
    }
  }
}

export async function handleMultiScanCompletion({
  multiResult,
  activeEnvUrl,
  activeProjectId,
  scanBackgrounded,
  scanContext,
  completeJob,
  loadHistory,
  loadLatestSessionSummary,
  activeScanScope,
  desktopNotificationsEnabled,
  refreshProjects,
  setScanFollowUpBanner,
  toast,
}: MultiScanCompletionParams): Promise<void> {
  const resultUrl = scanContext?.url ?? activeEnvUrl ?? null;
  const projectId = scanContext?.projectId ?? activeProjectId ?? null;
  // Prefer the persisted SiteCMD Score; raw page score is a degraded fallback.
  const completionScore = await loadCompletionScore(projectId, resultUrl, multiResult.overallScore);
  const latestSessionPromise = resultUrl
    ? loadLatestSessionSummary(projectId, resultUrl)
    : Promise.resolve<ScanSessionSummary | null>(null);
  const workflowCuePromise = loadPrimaryWorkflowCue(projectId, resultUrl);
  const copy = buildMultiScanCompletionCopy({
    score: completionScore.score,
    pageCount: multiResult.completedPages,
    scoreMessage: completionScore.scoreMessage,
    workflowCue: null,
  });
  const primaryTarget: AppTarget = {
    page: "issues",
    projectId,
    url: resultUrl,
    sessionId: null,
  };

  toast.success(copy.title, copy.body);
  completeJob("scan", {
    label: copy.jobLabel,
    scopeLabel: scanContext?.scopeLabel || activeScanScope || "Current site",
    detail: copy.jobDetail,
    target: primaryTarget,
  });

  const [latestSession, workflowCue] = await Promise.all([
    latestSessionPromise,
    workflowCuePromise,
  ]);
  const resolvedPrimaryTarget: AppTarget = {
    ...primaryTarget,
    sessionId: latestSession?.sessionId ?? null,
  };

  if (scanBackgrounded && desktopNotificationsEnabled) {
    await sendActionableDesktopNotification({
      id: `multi-scan:${resultUrl ?? "current"}:${multiResult.overallScore}:${multiResult.completedPages}`,
      title: copy.title,
      body: copy.body,
      clickTarget: resolvedPrimaryTarget,
      actions: buildScanResultNotificationActions({
        primaryTarget: resolvedPrimaryTarget,
        secondaryAction:
          getWorkflowNotificationFollowUpAction(workflowCue, resolvedPrimaryTarget) ??
          buildOpenTargetNotificationAction("open-dashboard", {
            page: "dashboard",
            projectId,
            url: resultUrl,
          }),
      }),
    }).catch(() => undefined);
  }

  refreshProjects();
  if (resultUrl) {
    await loadHistory(resultUrl, projectId ?? undefined);
    if (!scanBackgrounded) {
      setScanFollowUpBanner(null);
    }
  }
}
