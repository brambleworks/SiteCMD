import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CodeScanResult, CodeScanSummary, ScanResult } from "@/lib/types";

const {
  invokeMock,
  getProjectSignalSnapshotMock,
  primeLatestCodeScanSnapshotMock,
  getPrimaryWorkSummaryCueMock,
  buildCodeScanCompletionCopyMock,
  buildMultiScanCompletionCopyMock,
  buildWebScanCompletionCopyMock,
  buildOpenTargetNotificationActionMock,
  buildScanResultNotificationActionsMock,
  sendActionableDesktopNotificationMock,
  readOnboardingSetupStepsMock,
  removeOnboardingSetupStepMock,
  getPreviousCodeScanSummaryMock,
  buildCodeScanSummaryFromResultMock,
  describeCodeScanDomainTrendMock,
  loadCurrentScoreSnapshotMock,
  buildPostScanFollowUpBannerMock,
  getPreferredPostScanTargetMock,
  getWorkflowNotificationFollowUpActionMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  getProjectSignalSnapshotMock: vi.fn(),
  primeLatestCodeScanSnapshotMock: vi.fn(),
  getPrimaryWorkSummaryCueMock: vi.fn(),
  buildCodeScanCompletionCopyMock: vi.fn(),
  buildMultiScanCompletionCopyMock: vi.fn(),
  buildWebScanCompletionCopyMock: vi.fn(),
  buildOpenTargetNotificationActionMock: vi.fn(),
  buildScanResultNotificationActionsMock: vi.fn(),
  sendActionableDesktopNotificationMock: vi.fn(),
  readOnboardingSetupStepsMock: vi.fn(),
  removeOnboardingSetupStepMock: vi.fn(),
  getPreviousCodeScanSummaryMock: vi.fn(),
  buildCodeScanSummaryFromResultMock: vi.fn(),
  describeCodeScanDomainTrendMock: vi.fn(),
  loadCurrentScoreSnapshotMock: vi.fn(),
  buildPostScanFollowUpBannerMock: vi.fn(),
  getPreferredPostScanTargetMock: vi.fn(),
  getWorkflowNotificationFollowUpActionMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@/lib/project-summary-signals", () => ({
  getProjectSignalSnapshot: (...args: unknown[]) => getProjectSignalSnapshotMock(...args),
  primeLatestCodeScanSnapshot: (...args: unknown[]) => primeLatestCodeScanSnapshotMock(...args),
}));

vi.mock("@/lib/work-item-presentation", () => ({
  getPrimaryWorkSummaryCue: (...args: unknown[]) => getPrimaryWorkSummaryCueMock(...args),
}));

vi.mock("@/lib/scan-completion-copy", () => ({
  buildCodeScanCompletionCopy: (...args: unknown[]) => buildCodeScanCompletionCopyMock(...args),
  buildMultiScanCompletionCopy: (...args: unknown[]) => buildMultiScanCompletionCopyMock(...args),
  buildWebScanCompletionCopy: (...args: unknown[]) => buildWebScanCompletionCopyMock(...args),
}));

vi.mock("@/lib/notification-actions", () => ({
  buildOpenTargetNotificationAction: (...args: unknown[]) =>
    buildOpenTargetNotificationActionMock(...args),
  buildScanResultNotificationActions: (...args: unknown[]) =>
    buildScanResultNotificationActionsMock(...args),
}));

vi.mock("@/lib/actionable-notifications", () => ({
  sendActionableDesktopNotification: (...args: unknown[]) =>
    sendActionableDesktopNotificationMock(...args),
}));

vi.mock("@/lib/onboarding-setup", () => ({
  readOnboardingSetupSteps: (...args: unknown[]) => readOnboardingSetupStepsMock(...args),
  removeOnboardingSetupStep: (...args: unknown[]) => removeOnboardingSetupStepMock(...args),
}));

vi.mock("@/lib/code-scan-comparison", () => ({
  getPreviousCodeScanSummary: (...args: unknown[]) => getPreviousCodeScanSummaryMock(...args),
}));

vi.mock("@/lib/code-scan-summary-insights", () => ({
  buildCodeScanSummaryFromResult: (...args: unknown[]) =>
    buildCodeScanSummaryFromResultMock(...args),
  describeCodeScanDomainTrend: (...args: unknown[]) => describeCodeScanDomainTrendMock(...args),
}));

vi.mock("@/lib/scan-follow-up", () => ({
  buildPostScanFollowUpBanner: (...args: unknown[]) => buildPostScanFollowUpBannerMock(...args),
  getPreferredPostScanTarget: (...args: unknown[]) => getPreferredPostScanTargetMock(...args),
  getWorkflowNotificationFollowUpAction: (...args: unknown[]) =>
    getWorkflowNotificationFollowUpActionMock(...args),
}));

vi.mock("@/lib/current-score", () => ({
  currentScoreIssueCount: (score: {
    criticalCount: number;
    highCount: number;
    mediumCount: number;
    lowCount: number;
  }) => score.criticalCount + score.highCount + score.mediumCount + score.lowCount,
  loadCurrentScoreSnapshot: (...args: unknown[]) => loadCurrentScoreSnapshotMock(...args),
}));

import {
  handleCodeScanCompletion,
  handleFullMultiScanCompletion,
  handleFullScanCompletion,
  handleMultiScanCompletion,
  handleWebScanCompletion,
} from "./scan-completion-effects";

function buildCodeResult(overrides: Partial<CodeScanResult> = {}): CodeScanResult {
  return {
    id: 41,
    projectId: 7,
    environmentUrl: "https://scan.example.com",
    overallScore: 78,
    issueCount: 2,
    criticalCount: 0,
    highCount: 2,
    mediumCount: 0,
    lowCount: 0,
    durationMs: 1400,
    checkedAt: "2026-04-18T12:00:00Z",
    framework: "Next.js",
    domainSummaries: [],
    issues: [
      {
        id: "code-1",
        checkId: "code_scan.code-1",
        category: "security",
        domain: "security",
        severity: "high",
        title: "Unsafe query",
        description: "User input reaches raw SQL.",
        relativePath: "src/db/query.ts",
        absolutePath: "/tmp/project/src/db/query.ts",
        line: 42,
        sourceExcerpt: null,
        evidence: null,
        whyNow: null,
        likelyFix: null,
        confidence: "high",
        verifyHint: null,
      },
    ],
    ...overrides,
  };
}

function buildWebResult(overrides: Partial<ScanResult> = {}): ScanResult {
  return {
    url: "https://scan.example.com",
    mode: "live",
    scanType: "health",
    overallScore: 82,
    categories: [],
    issues: [],
    detectedStack: null,
    durationMs: 900,
    timestamp: "2026-04-18T12:00:00Z",
    ...overrides,
  };
}

function buildCodeHistoryEntry(overrides: Partial<CodeScanSummary> = {}): CodeScanSummary {
  return {
    id: 11,
    projectId: 7,
    environmentUrl: "https://scan.example.com",
    overallScore: 85,
    issueCount: 3,
    groupedIssueCount: 3,
    criticalCount: 0,
    highCount: 3,
    durationMs: 1100,
    checkedAt: "2026-04-17T12:00:00Z",
    framework: "Next.js",
    topDomain: "security",
    topDomainCount: 3,
    domainSummaries: [],
    ...overrides,
  };
}

describe("scan completion effects", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    getProjectSignalSnapshotMock.mockReset();
    primeLatestCodeScanSnapshotMock.mockReset();
    getPrimaryWorkSummaryCueMock.mockReset();
    buildCodeScanCompletionCopyMock.mockReset();
    buildMultiScanCompletionCopyMock.mockReset();
    buildWebScanCompletionCopyMock.mockReset();
    buildOpenTargetNotificationActionMock.mockReset();
    buildScanResultNotificationActionsMock.mockReset();
    sendActionableDesktopNotificationMock.mockReset();
    readOnboardingSetupStepsMock.mockReset();
    removeOnboardingSetupStepMock.mockReset();
    getPreviousCodeScanSummaryMock.mockReset();
    buildCodeScanSummaryFromResultMock.mockReset();
    describeCodeScanDomainTrendMock.mockReset();
    loadCurrentScoreSnapshotMock.mockReset();
    buildPostScanFollowUpBannerMock.mockReset();
    getPreferredPostScanTargetMock.mockReset();
    getWorkflowNotificationFollowUpActionMock.mockReset();

    getProjectSignalSnapshotMock.mockResolvedValue({ workSummary: null });
    getPrimaryWorkSummaryCueMock.mockReturnValue(null);
    getPreviousCodeScanSummaryMock.mockReturnValue(buildCodeHistoryEntry());
    buildCodeScanSummaryFromResultMock.mockReturnValue([]);
    describeCodeScanDomainTrendMock.mockReturnValue({ label: null });
    loadCurrentScoreSnapshotMock.mockRejectedValue(new Error("current score unavailable"));
    buildCodeScanCompletionCopyMock.mockReturnValue({
      title: "Code scan complete",
      body: "Code body",
      jobLabel: "Code scan",
      jobDetail: "Code detail",
    });
    buildWebScanCompletionCopyMock.mockReturnValue({
      title: "Web scan complete",
      body: "Web body",
      jobLabel: "Web scan",
      jobDetail: "Web detail",
    });
    buildMultiScanCompletionCopyMock.mockReturnValue({
      title: "Multi scan complete",
      body: "Multi body",
      jobLabel: "Multi scan",
      jobDetail: "Multi detail",
    });
    buildOpenTargetNotificationActionMock.mockReturnValue({ id: "open" });
    buildScanResultNotificationActionsMock.mockReturnValue([]);
    sendActionableDesktopNotificationMock.mockResolvedValue(undefined);
    readOnboardingSetupStepsMock.mockReturnValue([]);
    buildPostScanFollowUpBannerMock.mockReturnValue({ id: "banner" });
    getPreferredPostScanTargetMock.mockImplementation((_workflowCue, target) => target);
    getWorkflowNotificationFollowUpActionMock.mockReturnValue(null);
  });

  it("uses the captured project context when code scan completion reloads history", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const openAppTarget = vi.fn();

    await handleCodeScanCompletion({
      codeHistory: [buildCodeHistoryEntry()],
      codeResult: buildCodeResult(),
      activeEnvUrl: "https://other.example.com",
      activeProjectId: 99,
      currentProjectName: "Other project",
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(removeOnboardingSetupStepMock).toHaveBeenCalledWith(7, "code-scan");
    expect(loadHistory).toHaveBeenCalledWith("https://scan.example.com", 7);
    expect(openAppTarget).not.toHaveBeenCalled();
  });

  it("uses code scan domain summaries when the bridge returns a summary-only result", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);

    await handleCodeScanCompletion({
      codeHistory: [buildCodeHistoryEntry()],
      codeResult: buildCodeResult({
        issueCount: 9,
        criticalCount: 1,
        highCount: 2,
        mediumCount: 6,
        lowCount: 0,
        issues: [],
        domainSummaries: [
          {
            domain: "security",
            issueCount: 9,
            criticalCount: 1,
            highCount: 2,
            mediumCount: 6,
            lowCount: 0,
          },
        ],
      }),
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      currentProjectName: "Scan project",
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(buildCodeScanCompletionCopyMock).toHaveBeenCalledWith(
      expect.objectContaining({
        issueCount: 9,
        leadingDomain: {
          label: "Security",
          shortLabel: "Security",
          count: 9,
        },
      }),
    );
  });

  it("keeps foreground code scan completion in place for the summary overlay", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const openAppTarget = vi.fn();
    const setScanFollowUpBanner = vi.fn();

    await handleCodeScanCompletion({
      codeHistory: [buildCodeHistoryEntry()],
      codeResult: buildCodeResult(),
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      currentProjectName: "Scan project",
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner,
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(removeOnboardingSetupStepMock).toHaveBeenCalledWith(7, "code-scan");
    expect(setScanFollowUpBanner).toHaveBeenCalledWith(null);
    expect(getPreferredPostScanTargetMock).not.toHaveBeenCalled();
    expect(openAppTarget).not.toHaveBeenCalled();
  });

  it("lands the user in Issues after a project's first code scan turns up issues", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const openAppTarget = vi.fn();

    // No prior code pass for this project -> this is the baseline scan.
    getPreviousCodeScanSummaryMock.mockReturnValue(null);

    await handleCodeScanCompletion({
      codeHistory: [],
      codeResult: buildCodeResult(),
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      currentProjectName: "Scan project",
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(openAppTarget).toHaveBeenCalledWith(
      expect.objectContaining({ page: "issues", scanKind: "code", scanId: 41 }),
    );
  });

  it("marks the code scan job complete before waiting on workflow follow-up loading", async () => {
    const completeJob = vi.fn();
    const never = new Promise<never>(() => {});

    getProjectSignalSnapshotMock.mockReturnValue(never);

    void handleCodeScanCompletion({
      codeHistory: [buildCodeHistoryEntry()],
      codeResult: buildCodeResult(),
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      currentProjectName: "Scan project",
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob,
      loadHistory: vi.fn(),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    await vi.waitFor(() =>
      expect(completeJob).toHaveBeenCalledWith(
        "scan",
        expect.objectContaining({
          label: "Code scan",
          detail: "Code detail",
        }),
      ),
    );
  });

  it("reloads captured web scan history without auto-opening Issues", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const openAppTarget = vi.fn();

    await handleWebScanCompletion({
      result: buildWebResult(),
      history: [],
      activeEnvUrl: "https://other.example.com",
      activeProjectId: 99,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      loadLatestWebScanId: vi.fn().mockResolvedValue(321),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(loadHistory).toHaveBeenCalledWith("https://scan.example.com", 7);
    expect(openAppTarget).not.toHaveBeenCalled();
  });

  it("keeps redirected web completion scoped to the authored environment", async () => {
    const authoredUrl = "https://scan.example.com/start";
    const effectiveUrl = "https://scan.example.com/final";
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const loadLatestWebScanId = vi.fn().mockResolvedValue(321);
    const completeJob = vi.fn();
    const openAppTarget = vi.fn();

    await handleWebScanCompletion({
      result: buildWebResult({ url: effectiveUrl }),
      history: [{ url: authoredUrl, overallScore: 90, issuesTotal: 1 }],
      activeEnvUrl: "https://other.example.com",
      activeProjectId: 99,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: authoredUrl,
        scopeLabel: "Scan project",
      },
      completeJob,
      loadHistory,
      loadLatestWebScanId,
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(loadLatestWebScanId).toHaveBeenCalledWith(7, authoredUrl);
    expect(getProjectSignalSnapshotMock).toHaveBeenCalledWith(7, authoredUrl, {
      includeCodeScanDetail: false,
    });
    expect(loadHistory).toHaveBeenCalledWith(authoredUrl, 7);
    expect(completeJob).toHaveBeenCalledWith(
      "scan",
      expect.objectContaining({
        target: expect.objectContaining({ projectId: 7, url: authoredUrl }),
      }),
    );
    expect(openAppTarget).not.toHaveBeenCalled();
  });

  it("keeps foreground web scan completion in place for the summary overlay", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const openAppTarget = vi.fn();
    const setScanFollowUpBanner = vi.fn();

    await handleWebScanCompletion({
      result: buildWebResult(),
      history: [],
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      loadLatestWebScanId: vi.fn().mockResolvedValue(321),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner,
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(setScanFollowUpBanner).toHaveBeenCalledWith(null);
    expect(getPreferredPostScanTargetMock).not.toHaveBeenCalled();
    expect(openAppTarget).not.toHaveBeenCalled();
  });

  it("lands the user in Issues after a project's first web scan turns up issues", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const openAppTarget = vi.fn();

    await handleWebScanCompletion({
      result: buildWebResult({
        issues: [
          {
            checkId: "missing-alt-text",
            category: "accessibility",
            title: "Missing alt text",
            description: "Images need useful alternative text.",
            status: "fail",
            severity: "medium",
            fixPrompt: null,
            manualFix: null,
            rawData: null,
            confidence: "high",
          },
        ],
      }),
      // Empty history -> no prior scan for this URL -> baseline scan.
      history: [],
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      loadLatestWebScanId: vi.fn().mockResolvedValue(321),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(openAppTarget).toHaveBeenCalledWith(
      expect.objectContaining({ page: "issues", scanKind: "site", scanId: 321 }),
    );
  });

  it("does not yank the user to Issues when a web scan is a re-scan", async () => {
    const openAppTarget = vi.fn();

    await handleWebScanCompletion({
      result: buildWebResult({
        issues: [
          {
            checkId: "missing-alt-text",
            category: "accessibility",
            title: "Missing alt text",
            description: "Images need useful alternative text.",
            status: "fail",
            severity: "medium",
            fixPrompt: null,
            manualFix: null,
            rawData: null,
            confidence: "high",
          },
        ],
      }),
      // A prior scan for this URL exists -> re-scan, keep the user in place.
      history: [{ url: "https://scan.example.com", overallScore: 90, issuesTotal: 1 }],
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory: vi.fn().mockResolvedValue(undefined),
      loadLatestWebScanId: vi.fn().mockResolvedValue(321),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(openAppTarget).not.toHaveBeenCalled();
  });

  it("marks the web scan job complete before waiting on slow follow-up loading", async () => {
    const completeJob = vi.fn();
    const never = new Promise<never>(() => {});

    getProjectSignalSnapshotMock.mockReturnValue(never);

    void handleWebScanCompletion({
      result: buildWebResult(),
      history: [],
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob,
      loadHistory: vi.fn(),
      loadLatestWebScanId: vi.fn().mockReturnValue(never),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    await vi.waitFor(() =>
      expect(completeJob).toHaveBeenCalledWith(
        "scan",
        expect.objectContaining({
          label: "Web scan",
          detail: "Web detail",
        }),
      ),
    );
  });

  it("announces full scans with the persisted current SiteCMD score", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const toast = {
      success: vi.fn(),
      error: vi.fn(),
    };

    buildWebScanCompletionCopyMock.mockReturnValue({
      title: "Full scan complete",
      body: "Full scan body",
      jobLabel: "Full scan",
      jobDetail: "Full detail",
    });
    loadCurrentScoreSnapshotMock.mockResolvedValue({
      overall: 26,
      perCategory: {},
      criticalCount: 1,
      highCount: 2,
      mediumCount: 3,
      lowCount: 4,
      computedAt: 1,
    });

    await handleFullScanCompletion({
      result: buildWebResult({
        url: "https://scan.example.com/final",
        issues: [
          {
            checkId: "missing-alt-text",
            category: "accessibility",
            title: "Missing alt text",
            description: "Images need useful alternative text.",
            status: "fail",
            severity: "medium",
            fixPrompt: null,
            manualFix: null,
            rawData: null,
            confidence: "high",
          },
        ],
      }),
      codeResult: buildCodeResult({
        overallScore: 0,
      }),
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      currentProjectName: "Scan project",
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      loadLatestWebScanId: vi.fn().mockResolvedValue(321),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast,
    });

    expect(buildWebScanCompletionCopyMock).toHaveBeenCalledWith(
      expect.objectContaining({
        titleLabel: "Full Scan",
        jobLabel: "Full scan",
        score: 26,
        issueCount: 10,
      }),
    );
    expect(buildCodeScanCompletionCopyMock).not.toHaveBeenCalled();
    expect(toast.success).toHaveBeenCalledWith("Full scan complete", "Full scan body");
    expect(loadHistory).toHaveBeenCalledWith("https://scan.example.com", 7);
  });

  it("announces a multi-page full scan as a Full Scan", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const toast = {
      success: vi.fn(),
      error: vi.fn(),
    };

    buildWebScanCompletionCopyMock.mockReturnValue({
      title: "Full scan complete",
      body: "Full scan body",
      jobLabel: "Full scan",
      jobDetail: "Full detail",
    });
    loadCurrentScoreSnapshotMock.mockResolvedValue({
      overall: 26,
      perCategory: {},
      criticalCount: 1,
      highCount: 2,
      mediumCount: 3,
      lowCount: 4,
      computedAt: 1,
    });

    await handleFullMultiScanCompletion({
      multiResult: {
        overallScore: 80,
        completedPages: 3,
        pageResults: [{ issuesCount: 2 }, { issuesCount: 1 }],
        siteIssues: [{}],
      },
      codeResult: buildCodeResult(),
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      currentProjectName: "Scan project",
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      loadLatestSessionSummary: vi.fn().mockResolvedValue({ sessionId: 44 }),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast,
    });

    // Full Scan copy (unified snapshot score 26), never the Code Scan builder.
    expect(buildWebScanCompletionCopyMock).toHaveBeenCalledWith(
      expect.objectContaining({
        titleLabel: "Full Scan",
        jobLabel: "Full scan",
        score: 26,
      }),
    );
    expect(buildCodeScanCompletionCopyMock).not.toHaveBeenCalled();
    expect(toast.success).toHaveBeenCalledWith("Full scan complete", "Full scan body");
    expect(loadHistory).toHaveBeenCalledWith("https://scan.example.com", 7);
  });

  it("falls back to the summed multi-page and code issues when no snapshot is available", async () => {
    await handleFullMultiScanCompletion({
      multiResult: {
        overallScore: 80,
        completedPages: 3,
        pageResults: [{ issuesCount: 2 }, { issuesCount: 1 }],
        siteIssues: [{}],
      },
      codeResult: buildCodeResult({ overallScore: 55 }),
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      currentProjectName: "Scan project",
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory: vi.fn().mockResolvedValue(undefined),
      loadLatestSessionSummary: vi.fn().mockResolvedValue({ sessionId: 44 }),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(buildWebScanCompletionCopyMock).toHaveBeenCalledWith(
      expect.objectContaining({ score: 55, issueCount: 5 }),
    );
  });

  it("uses the captured project context for multi-scan history reloads", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const openAppTarget = vi.fn();

    await handleMultiScanCompletion({
      multiResult: {
        overallScore: 77,
        completedPages: 4,
      },
      activeEnvUrl: "https://other.example.com",
      activeProjectId: 99,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      loadLatestSessionSummary: vi.fn().mockResolvedValue({ sessionId: 44 }),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(loadHistory).toHaveBeenCalledWith("https://scan.example.com", 7);
    expect(openAppTarget).not.toHaveBeenCalled();
  });

  it("keeps foreground multi-scan completion in place for the summary overlay", async () => {
    const loadHistory = vi.fn().mockResolvedValue(undefined);
    const openAppTarget = vi.fn();
    const setScanFollowUpBanner = vi.fn();

    await handleMultiScanCompletion({
      multiResult: {
        overallScore: 77,
        completedPages: 4,
      },
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory,
      loadLatestSessionSummary: vi.fn().mockResolvedValue({ sessionId: 44 }),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget,
      refreshProjects: vi.fn(),
      setScanFollowUpBanner,
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(setScanFollowUpBanner).toHaveBeenCalledWith(null);
    expect(getPreferredPostScanTargetMock).not.toHaveBeenCalled();
    expect(openAppTarget).not.toHaveBeenCalled();
  });

  it("headlines the page-scan toast with the unified SiteCMD snapshot score, not the scan's own", async () => {
    // The persisted snapshot is the single SiteCMD Score; every completion toast
    // (web, code, full, and page scan) headlines it, never a per-scan number.
    loadCurrentScoreSnapshotMock.mockResolvedValue({
      overall: 26,
      perCategory: {},
      criticalCount: 1,
      highCount: 2,
      mediumCount: 3,
      lowCount: 4,
      computedAt: 1,
    });

    await handleMultiScanCompletion({
      multiResult: {
        overallScore: 77,
        completedPages: 4,
      },
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory: vi.fn().mockResolvedValue(undefined),
      loadLatestSessionSummary: vi.fn().mockResolvedValue({ sessionId: 44 }),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    // The unified snapshot score (26) headlines the toast, not multiResult's 77.
    expect(buildMultiScanCompletionCopyMock).toHaveBeenCalledWith(
      expect.objectContaining({ score: 26 }),
    );
    expect(buildMultiScanCompletionCopyMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ score: 77 }),
    );
  });

  it("falls back to the page scan's own score when no project snapshot is available", async () => {
    // Negative control: the default beforeEach rejects the snapshot load, so the
    // degraded path must reuse the page scan's own composite (77), not crash.
    await handleMultiScanCompletion({
      multiResult: {
        overallScore: 77,
        completedPages: 4,
      },
      activeEnvUrl: "https://scan.example.com",
      activeProjectId: 7,
      scanBackgrounded: false,
      scanContext: {
        projectId: 7,
        url: "https://scan.example.com",
        scopeLabel: "Scan project",
      },
      completeJob: vi.fn(),
      loadHistory: vi.fn().mockResolvedValue(undefined),
      loadLatestSessionSummary: vi.fn().mockResolvedValue({ sessionId: 44 }),
      activeScanScope: "Active scope",
      desktopNotificationsEnabled: false,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast: {
        success: vi.fn(),
        error: vi.fn(),
      },
    });

    expect(buildMultiScanCompletionCopyMock).toHaveBeenCalledWith(
      expect.objectContaining({ score: 77 }),
    );
  });
});
