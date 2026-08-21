import { useMemo, useState } from "react";
import { useQuery, type QueryClient } from "@tanstack/react-query";
import { getWorkItems } from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import type { IssueGroup } from "@/lib/types";
import { isInactiveIssueStatus } from "./active-issue-filter";

const EMPTY_KEYS: ReadonlySet<string> = new Set();

function deriveInactiveKeys(groups: IssueGroup[] | undefined): ReadonlySet<string> {
  if (!groups?.length) return EMPTY_KEYS;
  const inactive = new Set<string>();
  for (const group of groups) {
    if (isInactiveIssueStatus(group.status)) inactive.add(group.checkId);
  }
  return inactive;
}

function sameMembers(a: ReadonlySet<string>, b: ReadonlySet<string>): boolean {
  if (a === b) return true;
  if (a.size !== b.size) return false;
  for (const key of a) {
    if (!b.has(key)) return false;
  }
  return true;
}

/** Fetch fresh inactive keys through the shared query cache. */
export function fetchInactiveKeys(
  queryClient: QueryClient,
  projectId: number,
  normalizedUrl: string,
): Promise<ReadonlySet<string>> {
  return queryClient
    .fetchQuery({
      queryKey: queryKeys.workItems.forEnv(projectId, normalizedUrl),
      queryFn: () => getWorkItems({ projectId, envUrl: normalizedUrl }),
      staleTime: 0,
    })
    .then(deriveInactiveKeys);
}

/** Inactive lifecycle keys with stable identity while membership is unchanged. */
export function useInactiveIssueKeys(
  projectId: number,
  normalizedUrl: string,
): {
  groups: IssueGroup[];
  keys: ReadonlySet<string>;
  isLoading: boolean;
  isError: boolean;
  refetch: () => Promise<unknown>;
} {
  const query = useQuery({
    queryKey: queryKeys.workItems.forEnv(projectId, normalizedUrl),
    queryFn: () => getWorkItems({ projectId, envUrl: normalizedUrl }),
  });

  const derived = useMemo(() => deriveInactiveKeys(query.data), [query.data]);
  // State avoids publishing identities created by discarded renders.
  const [stable, setStable] = useState<ReadonlySet<string>>(derived);
  const keys = sameMembers(stable, derived) ? stable : derived;
  if (keys !== stable) setStable(keys);
  return {
    groups: query.data ?? [],
    keys,
    isLoading: query.isLoading,
    isError: query.isError,
    refetch: async () => {
      await query.refetch();
    },
  };
}
