import { useCallback, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getScanExecutions } from "@/lib/commands";
import { deriveScanPresentationHistory } from "@/lib/scan-execution-adapters";
import { queryKeys } from "@/lib/query/query-keys";
import type {
  CodeScanSummary,
  ScanExecutionSummary,
  ScanSessionSummary,
  ScanSummary,
} from "@/generated/ipc-bindings";

export type { ScanSessionSummary, ScanSummary } from "@/generated/ipc-bindings";

interface UseHistoryReturn {
  history: ScanSummary[];
  executions: ScanExecutionSummary[];
  codeHistory: CodeScanSummary[];
  sessions: ScanSessionSummary[];
  loading: boolean;
  historyError: string | null;
  loadHistory: (url: string, projectId?: number) => Promise<void>;
}

export function useHistory(): UseHistoryReturn {
  const queryClient = useQueryClient();
  const [scope, setScope] = useState<{
    projectId: number | null;
    environmentUrl: string | null;
  } | null>(null);
  const queryKey = queryKeys.scanExecution.history(
    scope?.projectId ?? null,
    scope?.environmentUrl ?? null,
    20,
  );
  const query = useQuery({
    queryKey,
    queryFn: () =>
      getScanExecutions({
        projectId: scope?.projectId ?? null,
        environmentUrl: scope?.environmentUrl ?? null,
        limit: 20,
      }),
    enabled: scope != null,
  });
  const executions = useMemo(() => query.data ?? [], [query.data]);
  const presentation = useMemo(() => deriveScanPresentationHistory(executions), [executions]);

  const loadHistory = useCallback(
    async (url: string, projectId?: number) => {
      if (!url && projectId == null) return;
      const nextScope = {
        projectId: projectId ?? null,
        environmentUrl: url || null,
      };
      setScope(nextScope);
      try {
        await queryClient.ensureQueryData({
          queryKey: queryKeys.scanExecution.history(
            nextScope.projectId,
            nextScope.environmentUrl,
            20,
          ),
          queryFn: () =>
            getScanExecutions({
              projectId: nextScope.projectId,
              environmentUrl: nextScope.environmentUrl,
              limit: 20,
            }),
        });
      } catch {
        // The observed query owns the error state. Keep this imperative API
        // non-throwing because page effects and completion handlers call it.
      }
    },
    [queryClient],
  );

  return {
    history: presentation.history,
    executions,
    codeHistory: presentation.codeHistory,
    sessions: presentation.sessions,
    loading: scope != null && query.isPending,
    historyError: query.isError ? "Scan history could not load." : null,
    loadHistory,
  };
}
