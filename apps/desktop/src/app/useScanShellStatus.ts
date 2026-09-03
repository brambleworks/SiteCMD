import { useEffect, useEffectEvent, useRef, type RefObject } from "react";
import { updateTrayScanStatus } from "@/lib/commands";
import { addJob, removeRunningJob } from "@/lib/jobs";
import type { ScanState } from "@/hooks/useScan";
import type { ScanRunStep } from "@/lib/scan-run-status";
import { getWebScanProgressDetail } from "@/lib/scan-progress-display";
import {
  getScanProgressSnapshot,
  readScanRunPercent,
  subscribeScanProgress,
} from "@/lib/scan-progress-store";
import { SCAN_LABELS } from "@/lib/scan-labels";
import type { ScheduledScanType } from "@/lib/types";
import { formatUrlDisplay } from "@/lib/utils";

/** The run model drifts between events, so the tray and job row re-read it on a clock too. */
const SHELL_PROGRESS_TICK_MS = 1_000;

export interface ScanJobContext {
  projectId: number | null;
  url: string | null;
  scopeLabel: string | null;
}

export function formatScanScopeLabel(projectName: string | null, url: string | null): string {
  const hostname = formatUrlDisplay(url);
  return projectName && hostname ? `${projectName} • ${hostname}` : hostname;
}

export function useScanShellStatus({
  activeEnvUrl,
  activeProjectId,
  activeScanScope,
  currentScanType,
  scanRunStep,
  scanJobContextRef,
  state,
}: {
  activeEnvUrl: string | null;
  activeProjectId: number | null;
  activeScanScope: string;
  currentScanType: ScheduledScanType | null;
  scanRunStep: ScanRunStep | null;
  scanJobContextRef: RefObject<ScanJobContext | null>;
  state: ScanState;
}) {
  const lastTrayPctRef = useRef(-1);

  // Effect events expose current render inputs without rebinding the progress subscription.
  const readInputs = useEffectEvent(() => ({
    activeEnvUrl,
    activeProjectId,
    activeScanScope,
    currentScanType,
    scanRunStep,
  }));

  useEffect(() => {
    if (state !== "scanning") {
      if (lastTrayPctRef.current !== -1) {
        lastTrayPctRef.current = -1;
        updateTrayScanStatus({ scanning: false }).catch(() => {});
      }
      if (state === "idle") {
        removeRunningJob("scan");
      }
      return;
    }

    let codePhaseSeen = false;
    const sync = () => {
      const { progress, multiProgress } = getScanProgressSnapshot();
      const { activeEnvUrl, activeProjectId, activeScanScope, currentScanType, scanRunStep } =
        readInputs();
      const scanContext = scanJobContextRef.current;
      const hostname = formatUrlDisplay(activeEnvUrl);
      codePhaseSeen =
        codePhaseSeen ||
        currentScanType === "code" ||
        Boolean(progress?.check_id.startsWith("code-scan."));
      const isCodeFamilyScan = codePhaseSeen;
      const activeRunStep =
        scanRunStep?.mode === "full" && codePhaseSeen
          ? {
              ...scanRunStep,
              stepIndex: scanRunStep.stepCount,
              label: SCAN_LABELS.code,
            }
          : scanRunStep;

      // One number for the ring, the job row, and the tray: the store's run model.
      const pct = Math.round(readScanRunPercent());
      if (Math.abs(pct - lastTrayPctRef.current) >= 5 || lastTrayPctRef.current === -1) {
        lastTrayPctRef.current = pct;
        updateTrayScanStatus({
          scanning: true,
          url: activeEnvUrl,
          pct,
        }).catch(() => {});
      }

      const detail = isCodeFamilyScan
        ? "Scanning project code…"
        : getWebScanProgressDetail(progress);
      const stepDetail =
        activeRunStep && activeRunStep.stepCount > 1
          ? `Step ${activeRunStep.stepIndex} of ${activeRunStep.stepCount}: ${activeRunStep.label}`
          : null;

      addJob({
        id: "scan",
        type: "scan",
        label:
          scanRunStep?.mode === "full"
            ? "Full scan"
            : currentScanType === "code"
              ? "Code scan"
              : multiProgress
                ? "Multi-page scan"
                : "Web scan",
        scopeLabel: scanContext?.scopeLabel || activeScanScope || hostname || "Current site",
        progress: pct,
        detail: stepDetail ?? detail,
        target: {
          page: "issues",
          restoreScan: true,
          projectId: scanContext?.projectId ?? activeProjectId,
          url: scanContext?.url ?? activeEnvUrl,
        },
      });
    };

    // Push the initial job/tray state immediately, then follow every event
    // and re-read the drifting model once a second in between.
    sync();
    const unsubscribe = subscribeScanProgress(sync);
    const interval = window.setInterval(sync, SHELL_PROGRESS_TICK_MS);
    return () => {
      unsubscribe();
      window.clearInterval(interval);
    };
  }, [state, scanJobContextRef]);
}
