import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  fetchGithubData,
  getCorrelations,
  getEvents,
  getGitStatus,
  getIntegrations,
} from "@/lib/commands";
import { getScanHistory } from "@/lib/scan-execution-adapters";
import { queryKeys } from "@/lib/query/query-keys";
import { MS_PER_DAY } from "@/lib/format";
import { useVisibilityRefresh } from "@/lib/useVisibilityRefresh";
import type { GitHubData } from "@/lib/analytics-types";
import type { SiteEvent } from "@/lib/types";
import type { DeployCorrelation, DeployScanSummary, GitStatus } from "./deploys-page-model";

interface DeploysOverviewData {
  gitStatus: GitStatus | null;
  scanHistory: DeployScanSummary[];
  deployEvents: SiteEvent[];
  correlations: DeployCorrelation[];
}

interface DeploysGithubData {
  data: GitHubData | null;
  configured: boolean;
  failed: boolean;
}

const DEPLOYS_OVERVIEW_REFRESH_MS = 60_000;
const DEPLOYS_GITHUB_STALE_MS = 5 * 60_000;

export function useDeploysPageData({
  projectId,
  projectPath,
  url,
}: {
  projectId: number;
  projectPath: string | null;
  url: string;
}) {
  const queryClient = useQueryClient();
  const overviewQuery = useQuery<DeploysOverviewData>({
    queryKey: queryKeys.deploys.overview(projectId, url, projectPath),
    queryFn: async () => {
      const endMs = Date.now();
      const startMs = endMs - 30 * MS_PER_DAY;
      const [gitStatus, scans, timeline, correlations] = await Promise.all([
        projectPath
          ? getGitStatus({ projectId, limit: 100 }).catch(() => null)
          : Promise.resolve(null),
        getScanHistory({ projectId, url, limit: 50 }).catch(() => []),
        getEvents({
          projectId,
          startMs,
          endMs,
          eventTypes: ["deploy"],
        }).catch(() => []),
        getCorrelations({ projectId }).catch(() => []) as Promise<DeployCorrelation[]>,
      ]);
      return {
        gitStatus,
        scanHistory: Array.isArray(scans) ? scans : [],
        deployEvents: Array.isArray(timeline) ? timeline : [],
        correlations: Array.isArray(correlations) ? correlations : [],
      };
    },
    staleTime: DEPLOYS_OVERVIEW_REFRESH_MS,
    refetchInterval: DEPLOYS_OVERVIEW_REFRESH_MS,
    refetchIntervalInBackground: false,
  });

  const githubQuery = useQuery<DeploysGithubData>({
    queryKey: queryKeys.deploys.github(projectId),
    queryFn: async () => {
      const configs = await queryClient.ensureQueryData({
        queryKey: queryKeys.integrations.forProject(projectId),
        queryFn: () => getIntegrations({ projectId }),
      });
      const configured = Array.isArray(configs)
        ? configs.some((config) => config.integrationType === "github")
        : false;
      try {
        return {
          data: await fetchGithubData<GitHubData>({ projectId }),
          configured,
          failed: false,
        };
      } catch {
        return { data: null, configured, failed: true };
      }
    },
    enabled: true,
    staleTime: DEPLOYS_GITHUB_STALE_MS,
  });

  useVisibilityRefresh({
    staleAfterMs: DEPLOYS_OVERVIEW_REFRESH_MS,
    onRefresh: () => void overviewQuery.refetch(),
  });
  useVisibilityRefresh({
    staleAfterMs: DEPLOYS_GITHUB_STALE_MS,
    onRefresh: () => void githubQuery.refetch(),
    enabled: true,
  });

  const reloadGithub = async () => {
    await queryClient.invalidateQueries({
      queryKey: queryKeys.integrations.forProject(projectId),
    });
    await githubQuery.refetch();
  };

  return {
    overview: overviewQuery.data ?? null,
    overviewLoading: overviewQuery.isPending,
    overviewRefreshing: overviewQuery.isFetching && !overviewQuery.isPending,
    github: githubQuery.data ?? null,
    githubLoading: githubQuery.isPending,
    reloadGithub,
  };
}
