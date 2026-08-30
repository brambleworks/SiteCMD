import { useCallback, useRef, useState } from "react";

import type { AppShellHooks } from "@/app/AppProviders";
import type { ScanJobContext } from "@/app/useScanShellStatus";
import type { ScanConfig, ScanConfigPreset } from "@/components/scan/ScanConfigOverlay";
import type { EnvironmentRecord, ProjectRecord } from "@/hooks/useProject";
import type { ScanPreferences } from "@/hooks/useScanPrefs";
import { getProjectCapabilities, NO_SITE_SCOPE_URL } from "@/lib/project-capabilities";
import { SCAN_LABELS } from "@/lib/scan-labels";
import { planScan } from "@/lib/scan-planner";
import type { ScanRunMode, ScanRunStep } from "@/lib/scan-run-status";
import { formatUrlDisplay } from "@/lib/utils";
import { createScanActionKey } from "@/lib/scan-action-key";

interface AppScanActionsOptions {
  activeEnv: EnvironmentRecord | null;
  activeProject: ProjectRecord | null;
  enabledCategories: string[];
  prefs: ScanPreferences;
  projectFolder: string | null;
  scanHook: AppShellHooks["scanHook"];
  toast: {
    error: (message: string, detail?: string) => void;
  };
}

export function useAppScanActions({
  activeEnv,
  activeProject,
  enabledCategories,
  prefs,
  projectFolder,
  scanHook,
  toast,
}: AppScanActionsOptions) {
  const { scan, scanExecution, state: scanState } = scanHook;
  const [showScanConfig, setShowScanConfig] = useState(false);
  const [scanConfigPreset, setScanConfigPreset] = useState<ScanConfigPreset | null>(null);
  const scanJobContextRef = useRef<ScanJobContext | null>(null);
  const [scanRunStep, setScanRunStep] = useState<ScanRunStep | null>(null);
  const [scanBackgrounded, setScanBackgrounded] = useState(false);
  const scanBackgroundedRef = useRef(false);
  const nextScanTriggerRef = useRef<"manual" | "tray">("manual");

  const updateScanBackgrounded = useCallback((next: boolean) => {
    scanBackgroundedRef.current = next;
    setScanBackgrounded(next);
  }, []);

  // Enforce one scan across buttons, shortcuts, and tray actions.
  const refuseWhileScanning = useCallback(() => {
    if (scanState !== "scanning") return false;
    toast.error(
      "A scan is already running",
      "Wait for it to finish, or cancel it, then start the next one.",
    );
    return true;
  }, [scanState, toast]);

  const openScanConfig = useCallback((preset?: ScanConfigPreset) => {
    nextScanTriggerRef.current = "manual";
    setScanConfigPreset(preset ?? null);
    setShowScanConfig(true);
  }, []);

  const openTrayScanConfig = useCallback(() => {
    nextScanTriggerRef.current = "tray";
    setScanConfigPreset(null);
    setShowScanConfig(true);
  }, []);

  const closeScanConfig = useCallback(() => {
    nextScanTriggerRef.current = "manual";
    setShowScanConfig(false);
    setScanConfigPreset(null);
  }, []);

  const showBackgroundedScan = useCallback(
    () => updateScanBackgrounded(false),
    [updateScanBackgrounded],
  );

  const handleScan = useCallback(
    async (config?: ScanConfig) => {
      if (refuseWhileScanning()) return;
      // Code-only projects have no environment but still have work to run.
      const capabilities = getProjectCapabilities({
        environmentUrl: activeEnv?.url ?? null,
        projectFolder,
      });
      if (!capabilities.hasSite && !capabilities.hasCode) return;
      updateScanBackgrounded(false);
      setShowScanConfig(false);
      setScanConfigPreset(null);
      const scopeUrl = activeEnv?.url ?? NO_SITE_SCOPE_URL;
      scanJobContextRef.current = {
        projectId: activeProject?.id ?? null,
        url: scopeUrl,
        scopeLabel: activeProject
          ? activeEnv
            ? `${activeProject.name} • ${formatUrlDisplay(activeEnv.url)}`
            : activeProject.name
          : formatUrlDisplay(scopeUrl),
      };

      const scanMode = (config?.scanType ?? "full") as ScanRunMode;
      const scanTrigger = nextScanTriggerRef.current;
      nextScanTriggerRef.current = "manual";
      const actions = planScan({
        mode: scanMode,
        urls: config?.urls,
        activeUrl: activeEnv?.url ?? null,
        activeProjectId: activeProject?.id ?? null,
        projectFolder,
        axeEnabled: config?.axeEnabled ?? false,
      });
      const validationError = actions.find((action) => action.kind === "error");
      if (validationError?.kind === "error") {
        setScanRunStep(null);
        toast.error(validationError.message, validationError.detail);
        return;
      }

      const urls = actions.flatMap((action) => {
        if (action.kind === "web-single") return [action.url];
        if (action.kind === "web-multi") return action.urls;
        return [];
      });
      const hasWebCollector = actions.some(
        (action) => action.kind === "web-single" || action.kind === "web-multi",
      );
      const hasCodeCollector = actions.some((action) => action.kind === "code");
      const collectorCount = Number(hasWebCollector) + Number(hasCodeCollector);
      setScanRunStep({
        mode: scanMode,
        stepIndex: 1,
        stepCount: Math.max(collectorCount, 1),
        label:
          !hasWebCollector && hasCodeCollector
            ? SCAN_LABELS.code
            : urls.length > 1
              ? SCAN_LABELS.multiPageWeb
              : SCAN_LABELS.web,
      });

      const outcome = await scanExecution({
        projectId: activeProject?.id ?? null,
        environmentId: activeEnv?.id ?? null,
        environmentUrl: activeEnv?.url ?? null,
        requestedMode: scanMode,
        webFocus: urls.length > 0 ? "health" : null,
        urls,
        enabledCategories,
        timeoutSecs: prefs.timeout,
        axeEnabled: config?.axeEnabled ?? false,
        inspectLocalDatabases: config?.inspectLocalDatabases ?? false,
        projectPath: projectFolder,
        retention: prefs.retentionLimit,
        trigger: scanTrigger,
        idempotencyKey: createScanActionKey(scanTrigger),
      });
      if (!outcome.ok) {
        setScanRunStep(null);
        return;
      }
    },
    [
      activeEnv,
      activeProject,
      enabledCategories,
      prefs.retentionLimit,
      prefs.timeout,
      projectFolder,
      refuseWhileScanning,
      scanExecution,
      toast,
      updateScanBackgrounded,
    ],
  );

  const handleQuickScan = useCallback(() => {
    void handleScan();
  }, [handleScan]);

  // Keyboard-shortcut scans bypass handleScan, so they stamp their own fresh
  // job context rather than leaking scope from whatever ran last.
  const handleShortcutScan = useCallback(
    (url: string, options?: { enabledCategories?: string[]; timeoutSecs?: number }) => {
      if (refuseWhileScanning()) {
        return Promise.resolve({ ok: false, error: "A scan is already running" } as const);
      }
      scanJobContextRef.current = {
        projectId: activeProject?.id ?? null,
        url,
        scopeLabel: activeProject
          ? `${activeProject.name} • ${formatUrlDisplay(url)}`
          : formatUrlDisplay(url),
      };
      return scan(url, {
        ...options,
        projectId: activeProject?.id ?? null,
        environmentId: activeEnv?.id ?? null,
        environmentUrl: activeEnv?.url ?? url,
        retention: prefs.retentionLimit,
        trigger: "manual",
      });
    },
    [activeEnv, activeProject, prefs.retentionLimit, refuseWhileScanning, scan],
  );

  return {
    closeScanConfig,
    handleQuickScan,
    handleScan,
    handleShortcutScan,
    openScanConfig,
    openTrayScanConfig,
    scanBackgrounded,
    scanBackgroundedRef,
    scanConfigPreset,
    scanJobContextRef,
    scanRunStep,
    showBackgroundedScan,
    showScanConfig,
    updateScanBackgrounded,
  };
}
