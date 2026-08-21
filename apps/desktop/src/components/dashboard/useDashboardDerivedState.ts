import { useMemo } from "react";

import { buildBootstrapTasks, type BootstrapInputs } from "@/lib/dashboard/bootstrap-tasks";
import { buildCriticalRollup } from "@/lib/dashboard/critical-rollup";
import type { CriticalRollup, SiteVerdict, SslProbeResult } from "@/lib/dashboard/types";
import { deriveSiteVerdict } from "@/lib/dashboard/verdict";
import type { DashboardWorkflowRun, SearchRegressionSignal } from "@/lib/project-summary-signals";
import { summarizeIssueSeverities } from "@/lib/issues";
import type { CodeScanResult, CodeScanSummary, PackageUpdate, ScanResult } from "@/lib/types";

interface UseDashboardDerivedStateOptions {
  aggregatedFailedIssues: ScanResult["issues"];
  configuredIntegrations: Set<string>;
  effectiveCodeScanDetail: CodeScanResult | null;
  integrationFailureCount: number;
  lastCIRun: DashboardWorkflowRun | null;
  latestCodeScanSummary: CodeScanSummary | null;
  projectPath: string | null;
  searchRegression: SearchRegressionSignal | null;
  securityUpdates: PackageUpdate[];
  sslProbe: SslProbeResult | null;
  staleIntegrationCount: number;
}

export function useDashboardDerivedState({
  aggregatedFailedIssues,
  configuredIntegrations,
  effectiveCodeScanDetail,
  integrationFailureCount,
  lastCIRun,
  latestCodeScanSummary,
  projectPath,
  searchRegression,
  securityUpdates,
  sslProbe,
  staleIntegrationCount,
}: UseDashboardDerivedStateOptions) {
  const webSeverityCounts = useMemo(
    () => summarizeIssueSeverities(aggregatedFailedIssues),
    [aggregatedFailedIssues],
  );
  const criticalWebIssues = useMemo(() => webSeverityCounts.critical, [webSeverityCounts.critical]);
  const criticalCodeIssues = useMemo(() => {
    const detailCount = summarizeIssueSeverities(effectiveCodeScanDetail?.issues ?? []).critical;
    if (detailCount > 0) return detailCount;
    return latestCodeScanSummary?.criticalCount ?? 0;
  }, [effectiveCodeScanDetail, latestCodeScanSummary]);
  const highWebIssues = useMemo(() => webSeverityCounts.high, [webSeverityCounts.high]);

  const criticalRollup = useMemo<CriticalRollup>(
    () =>
      buildCriticalRollup({
        criticalWebIssues,
        criticalCodeIssues,
        securityPatchCount: securityUpdates.length,
      }),
    [criticalWebIssues, criticalCodeIssues, securityUpdates.length],
  );

  const verdict = useMemo<SiteVerdict>(
    () =>
      deriveSiteVerdict({
        criticalWebIssues,
        criticalCodeIssues,
        securityPatchCount: securityUpdates.length,
        highWebIssues,
        deployFailed: lastCIRun?.conclusion === "failure",
        integrationFailureCount,
        staleIntegrationCount,
        searchRegressionNegative: (searchRegression?.deltaPct ?? 0) < 0,
        sslDaysRemaining: sslProbe?.days_remaining ?? null,
      }),
    [
      criticalWebIssues,
      criticalCodeIssues,
      securityUpdates.length,
      highWebIssues,
      lastCIRun,
      integrationFailureCount,
      staleIntegrationCount,
      searchRegression,
      sslProbe,
    ],
  );

  const bootstrapInputs = useMemo<BootstrapInputs>(
    () => ({
      hasProjectFolder: Boolean(projectPath),
      hasCodeScan: latestCodeScanSummary != null,
      hasSchedule: false, // wired in a later task
      hasAnalytics: ["plausible", "googleanalytics", "cloudflare"].some((type) =>
        configuredIntegrations.has(type),
      ),
      hasUptime: configuredIntegrations.has("uptimerobot"),
      hasSearch: ["googlesearchconsole", "bingwebmaster"].some((type) =>
        configuredIntegrations.has(type),
      ),
      hasGithub: configuredIntegrations.has("github"),
      hasReportSchedule: false,
      mcpConfigured: false,
    }),
    [projectPath, latestCodeScanSummary, configuredIntegrations],
  );

  const bootstrapTasks = useMemo(() => buildBootstrapTasks(bootstrapInputs), [bootstrapInputs]);

  return {
    bootstrapTasks,
    criticalCodeIssues,
    criticalRollup,
    criticalWebIssues,
    highWebIssues,
    verdict,
  };
}
