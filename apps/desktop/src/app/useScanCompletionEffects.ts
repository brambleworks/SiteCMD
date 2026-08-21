import { useEffect, useRef, type Dispatch, type RefObject, type SetStateAction } from "react";

import { loadLatestSessionSummary, loadLatestWebScanId } from "@/app/scan-history-lookups";
import type { ScanJobContext } from "@/app/useScanShellStatus";
import type { MultiScanResult, ScanState } from "@/hooks/useScan";
import type { ScanSummary } from "@/hooks/useHistory";
import type { ScanRunStep } from "@/lib/scan-run-status";
import type { AppTarget } from "@/lib/app-targets";
import { completeJob, failJob } from "@/lib/jobs";
import { getScanProgressSnapshot } from "@/lib/scan-progress-store";
import {
  handleCodeScanCompletion,
  handleFullMultiScanCompletion,
  handleFullScanCompletion,
  handleMultiScanCompletion,
  handleWebScanCompletion,
} from "@/lib/scan-completion-effects";
import { markFirstScanCompleted } from "@/lib/onboarding-flags";
import { formatScanError, parseScanError } from "@/lib/scan-error";
import type { PostScanFollowUpBanner } from "@/lib/scan-follow-up";
import type { CodeScanResult, CodeScanSummary, ScanResult, ScheduledScanType } from "@/lib/types";

interface ToastApi {
  success: (title: string, body?: string) => void;
  error: (title: string, body?: string) => void;
}

interface UseScanCompletionEffectsParams {
  state: ScanState;
  currentScanType: ScheduledScanType | null;
  result: ScanResult | null;
  codeResult: CodeScanResult | null;
  multiResult: MultiScanResult | null;
  error: string | null;
  activeEnvUrl: string | null | undefined;
  activeProjectId: number | null | undefined;
  activeProjectName: string | null | undefined;
  activeScanScope: string;
  history: ScanSummary[];
  codeHistory: CodeScanSummary[];
  scanRunStep: ScanRunStep | null;
  scanBackgroundedRef: RefObject<boolean>;
  scanJobContextRef: RefObject<ScanJobContext | null>;
  desktopNotificationsEnabled: boolean;
  loadHistory: (url: string, projectId?: number) => Promise<void>;
  openAppTarget: (target: AppTarget) => void;
  refreshProjects: () => void;
  setScanFollowUpBanner: Dispatch<SetStateAction<PostScanFollowUpBanner | null>>;
  toast: ToastApi;
}

export function useScanCompletionEffects({
  state,
  currentScanType,
  result,
  codeResult,
  multiResult,
  error,
  activeEnvUrl,
  activeProjectId,
  activeProjectName,
  activeScanScope,
  history,
  codeHistory,
  scanRunStep,
  scanBackgroundedRef,
  scanJobContextRef,
  desktopNotificationsEnabled,
  loadHistory,
  openAppTarget,
  refreshProjects,
  setScanFollowUpBanner,
  toast,
}: UseScanCompletionEffectsParams) {
  const handledScanRef = useRef<string | null>(null);

  useEffect(() => {
    if (state !== "complete" && state !== "error") {
      handledScanRef.current = null;
      return;
    }

    // Either web result shape identifies a Full Scan completion.
    const isFullSingleCompletion = scanRunStep?.mode === "full" && Boolean(result && codeResult);
    const isFullMultiCompletion =
      scanRunStep?.mode === "full" && !result && Boolean(multiResult && codeResult);
    const scanKey =
      isFullSingleCompletion && result && codeResult
        ? `full-${result.url}-${result.overallScore}-${codeResult.id}-${codeResult.overallScore}-${codeResult.issueCount}`
        : isFullMultiCompletion && multiResult && codeResult
          ? `full-multi-${multiResult.overallScore}-${multiResult.completedPages}-${codeResult.id}-${codeResult.overallScore}-${codeResult.issueCount}`
          : codeResult
            ? `code-${codeResult.id}-${codeResult.overallScore}-${codeResult.issueCount}`
            : result
              ? `single-${result.url}-${result.overallScore}`
              : multiResult
                ? `multi-${multiResult.overallScore}-${multiResult.completedPages}`
                : error
                  ? `error-${error}`
                  : null;

    if (!scanKey || handledScanRef.current === scanKey) return;
    handledScanRef.current = scanKey;

    if (state === "complete" && (result || codeResult || multiResult)) {
      markFirstScanCompleted();
    }

    if (state === "complete" && isFullSingleCompletion && result && codeResult) {
      void handleFullScanCompletion({
        result,
        codeResult,
        activeEnvUrl,
        activeProjectId,
        currentProjectName: activeProjectName,
        scanBackgrounded: scanBackgroundedRef.current,
        scanContext: scanJobContextRef.current,
        completeJob,
        loadHistory,
        loadLatestWebScanId,
        activeScanScope,
        desktopNotificationsEnabled,
        openAppTarget,
        refreshProjects,
        setScanFollowUpBanner,
        toast,
      });
      return;
    }

    if (state === "complete" && isFullMultiCompletion && multiResult && codeResult) {
      void handleFullMultiScanCompletion({
        multiResult,
        codeResult,
        activeEnvUrl,
        activeProjectId,
        currentProjectName: activeProjectName,
        scanBackgrounded: scanBackgroundedRef.current,
        scanContext: scanJobContextRef.current,
        completeJob,
        loadHistory,
        loadLatestSessionSummary,
        activeScanScope,
        desktopNotificationsEnabled,
        openAppTarget,
        refreshProjects,
        setScanFollowUpBanner,
        toast,
      });
      return;
    }

    if (state === "complete" && codeResult) {
      void handleCodeScanCompletion({
        codeHistory,
        codeResult,
        activeEnvUrl,
        activeProjectId,
        currentProjectName: activeProjectName,
        scanBackgrounded: scanBackgroundedRef.current,
        scanContext: scanJobContextRef.current,
        completeJob,
        loadHistory,
        activeScanScope,
        desktopNotificationsEnabled,
        openAppTarget,
        refreshProjects,
        setScanFollowUpBanner,
        toast,
      });
      return;
    }

    if (state === "complete" && result) {
      void handleWebScanCompletion({
        result,
        history,
        activeEnvUrl,
        activeProjectId,
        scanBackgrounded: scanBackgroundedRef.current,
        scanContext: scanJobContextRef.current,
        completeJob,
        loadHistory,
        loadLatestWebScanId,
        activeScanScope,
        desktopNotificationsEnabled,
        openAppTarget,
        refreshProjects,
        setScanFollowUpBanner,
        toast,
      });
      return;
    }

    if (state === "complete" && multiResult) {
      void handleMultiScanCompletion({
        multiResult,
        activeEnvUrl,
        activeProjectId,
        scanBackgrounded: scanBackgroundedRef.current,
        scanContext: scanJobContextRef.current,
        completeJob,
        loadHistory,
        loadLatestSessionSummary,
        activeScanScope,
        desktopNotificationsEnabled,
        openAppTarget,
        refreshProjects,
        setScanFollowUpBanner,
        toast,
      });
      return;
    }

    if (state === "error" && error) {
      const scanContext = scanJobContextRef.current;
      const formatted = formatScanError(parseScanError(error));
      // The retained progress snapshot distinguishes failed multi-page scans.
      const wasMultiPage = getScanProgressSnapshot().multiProgress != null;
      failJob("scan", {
        label:
          currentScanType === "code" ? "Code scan" : wasMultiPage ? "Multi-page scan" : "Web scan",
        scopeLabel: scanContext?.scopeLabel || activeScanScope || "Current site",
        detail: formatted.body,
        target: {
          page: "issues",
          projectId: scanContext?.projectId ?? activeProjectId,
          url: scanContext?.url ?? activeEnvUrl,
        },
      });
      toast.error(formatted.title, formatted.body);
    }
  }, [
    state,
    codeResult,
    codeHistory,
    result,
    multiResult,
    error,
    activeEnvUrl,
    activeProjectId,
    activeProjectName,
    activeScanScope,
    history,
    loadHistory,
    refreshProjects,
    toast,
    scanRunStep,
    currentScanType,
    desktopNotificationsEnabled,
    openAppTarget,
    setScanFollowUpBanner,
    scanBackgroundedRef,
    scanJobContextRef,
  ]);
}
