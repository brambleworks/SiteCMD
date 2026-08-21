import { useCallback, useEffect, useState } from "react";

interface BaselineScanQueueOptions {
  activeProjectId: number | null;
  /** True once the active project has a site, a codebase, or both. */
  canScan: boolean;
  runBaselineScan: () => void;
}

// Queue baseline scans until the created project is active in the current
// render. Readiness includes code-only projects without an environment.
export function useBaselineScanQueue({
  activeProjectId,
  canScan,
  runBaselineScan,
}: BaselineScanQueueOptions) {
  const [pendingProjectId, setPendingProjectId] = useState<number | null>(null);

  const queueBaselineScan = useCallback((projectId: number) => {
    setPendingProjectId(projectId);
  }, []);

  useEffect(() => {
    if (pendingProjectId == null) return;
    if (activeProjectId !== pendingProjectId) return;
    if (!canScan) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- clears the one-shot pending flag before kicking off the queued baseline scan; an imperative trigger
    setPendingProjectId(null);
    runBaselineScan();
  }, [activeProjectId, canScan, pendingProjectId, runBaselineScan]);

  return { queueBaselineScan };
}
