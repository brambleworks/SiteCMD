import { useCallback } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { generateReportData } from "./report-commands";
import { queryKeys } from "@/lib/query/query-keys";
import {
  applyReportPresentation,
  sectionsSignature,
  type Branding,
  type ReportData,
  type SectionConfig,
} from "@/components/reports/reports-page-model";

interface ReportBuildOverrides {
  branding?: Branding;
  reportTitle?: string;
  sections?: SectionConfig;
  projectId?: number;
  siteUrl?: string;
  periodDays?: number;
}

interface UseReportSnapshotOptions {
  branding: Branding;
  periodDays: number;
  projectId: number | null;
  reportTitle: string;
  sections: SectionConfig;
  siteUrl: string;
}

function reportSnapshotQuery({
  periodDays,
  projectId,
  sections,
  siteUrl,
}: {
  periodDays: number;
  projectId: number;
  sections: SectionConfig;
  siteUrl: string;
}) {
  return {
    queryKey: queryKeys.reports.snapshot(
      projectId,
      siteUrl,
      periodDays,
      sectionsSignature(sections),
    ),
    queryFn: () => generateReportData({ projectId, siteUrl, periodDays, sections }),
  };
}

export function useReportSnapshot({
  branding,
  periodDays,
  projectId,
  reportTitle,
  sections,
  siteUrl,
}: UseReportSnapshotOptions) {
  const queryClient = useQueryClient();
  const enabled = projectId != null && Boolean(siteUrl);
  const query = useQuery<ReportData>({
    ...reportSnapshotQuery({
      projectId: projectId ?? 0,
      siteUrl,
      periodDays,
      sections,
    }),
    enabled,
  });

  const loadSnapshot = useCallback(async () => {
    if (!enabled) return;
    await query.refetch();
  }, [enabled, query]);

  const buildConfiguredReportData = useCallback(
    async (overrides?: ReportBuildOverrides) => {
      const resolvedBranding = overrides?.branding ?? branding;
      const resolvedProjectId = overrides?.projectId ?? projectId;
      const resolvedSiteUrl = overrides?.siteUrl ?? siteUrl;
      const resolvedPeriodDays = overrides?.periodDays ?? periodDays;
      const resolvedSections = overrides?.sections ?? sections;
      if (resolvedProjectId == null || !resolvedSiteUrl) {
        throw new Error("No site selected");
      }
      const data = await queryClient.ensureQueryData(
        reportSnapshotQuery({
          projectId: resolvedProjectId,
          siteUrl: resolvedSiteUrl,
          periodDays: resolvedPeriodDays,
          sections: resolvedSections,
        }),
      );
      return applyReportPresentation(data, {
        branding: resolvedBranding,
        reportTitle: overrides?.reportTitle ?? reportTitle,
        sections: resolvedSections,
      });
    },
    [branding, periodDays, projectId, queryClient, reportTitle, sections, siteUrl],
  );

  const ensureHistorySummary = useCallback(async () => {
    if (projectId == null || !siteUrl) throw new Error("No site selected");
    return queryClient.ensureQueryData(
      reportSnapshotQuery({ projectId, siteUrl, periodDays, sections }),
    );
  }, [periodDays, projectId, queryClient, sections, siteUrl]);

  return {
    buildConfiguredReportData,
    ensureHistorySummary,
    loadSnapshot,
    reportSnapshot: query.data ?? null,
    snapshotError:
      enabled && query.isError ? "The latest report snapshot could not be loaded." : null,
    snapshotLoading: enabled && query.isPending,
    snapshotRefreshing: query.isFetching && !query.isPending,
  };
}
