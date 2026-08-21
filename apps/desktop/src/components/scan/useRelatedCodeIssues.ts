import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { runCodeScanAudit } from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import { severityRank } from "@/lib/severity";
import type { CodeIssue, FixLocation } from "@/lib/types";

const EMPTY_CODE_ISSUES: CodeIssue[] = [];
// Reuse recent audits; scan completion events still invalidate the query family.
const CODE_SCAN_AUDIT_STALE_MS = 15 * 60 * 1000;

interface UseRelatedCodeIssuesArgs {
  correlatedFiles: FixLocation[];
  projectId?: number;
  projectPath?: string;
}

/** Derives related Code Scan findings from shared audit data and fix locations. */
export function useRelatedCodeIssues({
  correlatedFiles,
  projectId,
  projectPath,
}: UseRelatedCodeIssuesArgs): CodeIssue[] {
  // correlatedFiles is state-held upstream, so this Set stays reference-stable
  // between fix-location changes and the derived result below does not churn.
  const matchedPaths = useMemo(
    () => new Set(correlatedFiles.map((file) => file.relativePath.toLowerCase())),
    [correlatedFiles],
  );
  const enabled = Boolean(projectId && projectPath && matchedPaths.size > 0);

  const { data: report } = useQuery({
    queryKey: queryKeys.codeScanAudit.forProject(projectId ?? -1, projectPath ?? ""),
    queryFn: () =>
      runCodeScanAudit({
        projectId: projectId!,
        projectPath: projectPath!,
        inspectLocalDatabases: false,
      }),
    enabled,
    staleTime: CODE_SCAN_AUDIT_STALE_MS,
  });

  return useMemo(() => {
    if (!enabled || !report) return EMPTY_CODE_ISSUES;
    return report.issues
      .filter((candidate) => matchedPaths.has(candidate.relativePath.toLowerCase()))
      .sort((a, b) => severityRank(a.severity) - severityRank(b.severity))
      .slice(0, 4);
  }, [enabled, matchedPaths, report]);
}
