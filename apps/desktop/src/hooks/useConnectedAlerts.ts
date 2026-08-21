import { useQuery } from "@tanstack/react-query";
import type { ConnectedAlertFeed } from "@/generated/ipc-bindings-connected";
import { listConnectedAlerts } from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";

// Missing transport data means no connected service; rejected reads remain errors.
const NO_SERVICE: ConnectedAlertFeed = {
  alerts: [],
  availability: "service_unconfigured",
  elsewhere: [],
  truncated: false,
};

export interface ConnectedAlertsState {
  feed: ConnectedAlertFeed;
  loading: boolean;
  /** Whether the read failed rather than returned an empty feed. */
  failed: boolean;
}

/** Read connected alerts for one project environment. */
export function useConnectedAlerts(
  projectId: number | null,
  environmentScopeKey: string,
): ConnectedAlertsState {
  const query = useQuery({
    queryKey: queryKeys.alerts.connected(projectId ?? 0, environmentScopeKey),
    queryFn: async () => {
      const feed = await listConnectedAlerts({
        environmentScopeKey,
        projectId: projectId as number,
      });
      return feed ?? NO_SERVICE;
    },
    enabled: projectId != null,
  });

  return {
    failed: query.isError,
    feed: query.data ?? NO_SERVICE,
    loading: projectId != null && query.isPending,
  };
}
