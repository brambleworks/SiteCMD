import { useCallback } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { deleteReportHistory, getReportHistory, saveReportHistory } from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import {
  buildReportHistorySummary,
  toPersistedBranding,
  type Branding,
  type ReportData,
  type ReportHistoryEntry,
  type SectionConfig,
} from "@/components/reports/reports-page-model";
import { userFacingError } from "@/lib/user-facing-error";

interface ReportsHistoryToast {
  error: (title: string, message?: string) => void;
  success: (title: string, message?: string) => void;
}

interface UseReportsHistoryOptions {
  branding: Branding;
  ensureHistorySummary: () => Promise<ReportData>;
  periodDays: number;
  projectId: number | null;
  reportTitle: string;
  sections: SectionConfig;
  siteUrl: string;
  toast: ReportsHistoryToast;
}

export function useReportsHistory({
  branding,
  ensureHistorySummary,
  periodDays,
  projectId,
  reportTitle,
  sections,
  siteUrl,
  toast,
}: UseReportsHistoryOptions) {
  const queryClient = useQueryClient();
  const queryKey = queryKeys.reports.history(projectId ?? 0);
  const historyQuery = useQuery<ReportHistoryEntry[]>({
    queryKey,
    queryFn: async () => {
      const entries = await getReportHistory({ projectId: projectId as number });
      return Array.isArray(entries) ? (entries as ReportHistoryEntry[]) : [];
    },
    enabled: projectId != null,
  });

  const loadHistory = useCallback(async () => {
    if (projectId == null) return;
    await historyQuery.refetch();
  }, [historyQuery, projectId]);

  const recordReportHistory = useCallback(
    async (outputFormat: "preview" | "html" | "pdf", summarySource?: ReportData) => {
      if (!projectId || !siteUrl) return;
      const source = summarySource ?? (await ensureHistorySummary());
      await saveReportHistory({
        projectId,
        siteUrl,
        periodDays,
        reportTitle: reportTitle || "Site & Code Report",
        outputFormat,
        brandingJson: JSON.stringify(toPersistedBranding(branding)),
        sectionsJson: JSON.stringify(sections),
        reportSummaryJson: JSON.stringify(buildReportHistorySummary(source)),
      });
      await queryClient.invalidateQueries({ queryKey });
    },
    [
      branding,
      ensureHistorySummary,
      periodDays,
      projectId,
      queryClient,
      queryKey,
      reportTitle,
      sections,
      siteUrl,
    ],
  );

  const deleteHistoryReport = useCallback(
    async (entry: ReportHistoryEntry) => {
      try {
        await deleteReportHistory({ id: entry.id });
        await queryClient.invalidateQueries({ queryKey });
        toast.success("Report deleted", "Removed from history");
      } catch (error) {
        toast.error(
          "Delete failed",
          userFacingError(error, "Your change was not saved. Try again."),
        );
      }
    },
    [queryClient, queryKey, toast],
  );

  return {
    deleteHistoryReport,
    history: historyQuery.data ?? [],
    historyError: historyQuery.isError ? "Report history could not be loaded." : null,
    historyLoading: projectId != null && historyQuery.isPending,
    historyRefreshing: historyQuery.isFetching && !historyQuery.isPending,
    loadHistory,
    recordReportHistory,
  };
}
