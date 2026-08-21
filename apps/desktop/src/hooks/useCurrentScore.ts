import { useCallback } from "react";
import { loadCurrentScoreSnapshot } from "@/lib/current-score";
import type { ScoreSnapshot } from "@/lib/types";
import { useQuery } from "@tanstack/react-query";
import { queryKeys } from "@/lib/query/query-keys";

export function useCurrentScore(projectId: number | null, envUrl: string | null) {
  const normalizedEnvUrl = envUrl ?? "";
  const query = useQuery<ScoreSnapshot | null>({
    queryKey: queryKeys.currentScore.forEnv(projectId, normalizedEnvUrl),
    queryFn: () => loadCurrentScoreSnapshot(projectId as number, envUrl),
    enabled: projectId != null,
  });

  // Callers use `refresh` as an effect dependency, so its identity must stay stable.
  const { refetch } = query;
  const refresh = useCallback(async () => {
    await refetch();
  }, [refetch]);

  return {
    score: query.data ?? null,
    loading: projectId != null && query.isPending,
    refreshing: query.isFetching && !query.isPending,
    error: query.isError ? String(query.error) : null,
    refresh,
  };
}
