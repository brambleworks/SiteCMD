import { useCallback, useMemo } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getIntegrations } from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import type { IntegrationConfig } from "@/components/settings/integration-services";

export function useIntegrationsQuery(projectId: number) {
  const queryClient = useQueryClient();
  const query = useQuery<IntegrationConfig[]>({
    queryKey: queryKeys.integrations.forProject(projectId),
    queryFn: async () => {
      const configs = await getIntegrations({ projectId });
      return Array.isArray(configs) ? (configs as IntegrationConfig[]) : [];
    },
  });
  const refetchIntegrations = query.refetch;
  const configs = useMemo(() => query.data ?? [], [query.data]);

  const reload = useCallback(async () => {
    await queryClient.invalidateQueries({
      queryKey: queryKeys.integrations.forProject(projectId),
      refetchType: "none",
    });
    const result = await refetchIntegrations();
    return result.data ?? [];
  }, [projectId, queryClient, refetchIntegrations]);

  return {
    configs,
    loading: query.isPending,
    refreshing: query.isFetching && !query.isPending,
    error: query.isError ? "Integrations could not load right now." : null,
    reload,
  };
}
