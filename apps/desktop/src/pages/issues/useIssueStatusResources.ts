import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { getWorkItems } from "@/lib/commands";
import type { IssueStatusFilter } from "@/components/issues/IssueList";
import type { ProjectWorkItem } from "@/lib/project-summary-types";
import type { IssueGroup } from "@/lib/types";
import { getResolvedIssues, type ResolvedIssue } from "@/lib/resolved-issues";
import { queryKeys } from "@/lib/query/query-keys";

const EMPTY_PAUSED_WORK_ITEMS: ProjectWorkItem[] = [];
const EMPTY_RESOLVED_ISSUES: ResolvedIssue[] = [];

type WorkItemIssueStatusFilter = Extract<IssueStatusFilter, "ignored" | "blocked" | "all">;

function resolveStatusFilter(statusFilter: IssueStatusFilter): WorkItemIssueStatusFilter | null {
  if (statusFilter === "all" || statusFilter === "ignored" || statusFilter === "blocked") {
    return statusFilter;
  }
  return null;
}

// Paused rows retain a synthetic target for the ProjectWorkItem wire shape.
function groupToPausedRow(group: IssueGroup, projectId: number, url: string): ProjectWorkItem {
  return {
    stableKey: group.checkId,
    projectId,
    environmentUrl: url,
    kind: group.sources.includes("code_scan") ? "code" : "web",
    status: group.status,
    severity: group.severity,
    title: group.title,
    summary: group.description,
    category: group.category,
    domain: null,
    packageName: null,
    target: { page: "issues", projectId, url, itemId: group.checkId },
    firstSeenAt: "",
    lastSeenAt: "",
    lastVerifiedAt: null,
    lastStatusChangedAt: "",
  };
}

// Paused tabs use the same grouped lifecycle source as the active list and score.
export function useIssueStatusResources({
  projectId,
  normalizedUrl,
}: {
  projectId: number;
  normalizedUrl: string;
}) {
  const [statusFilter, setStatusFilter] = useState<IssueStatusFilter>("active");
  const pausedFilter = resolveStatusFilter(statusFilter);
  const pausedQuery = useQuery({
    queryKey: queryKeys.workItems.forEnv(projectId, normalizedUrl),
    queryFn: () => getWorkItems({ projectId, envUrl: normalizedUrl }),
    enabled: pausedFilter != null,
  });
  const resolvedQuery = useQuery<ResolvedIssue[]>({
    queryKey: queryKeys.resolvedIssues.forEnv(projectId, normalizedUrl, 100),
    // Fetch with the normalized URL used by the cache key.
    queryFn: () => getResolvedIssues(projectId, normalizedUrl, 100),
    enabled: statusFilter === "resolved",
  });
  const pausedWorkItems = useMemo(
    () =>
      pausedFilter && pausedQuery.data
        ? pausedQuery.data
            .filter((group) =>
              pausedFilter === "all"
                ? group.status === "blocked" || group.status === "ignored"
                : group.status === pausedFilter,
            )
            .map((group) => groupToPausedRow(group, projectId, normalizedUrl))
        : EMPTY_PAUSED_WORK_ITEMS,
    [normalizedUrl, pausedFilter, pausedQuery.data, projectId],
  );
  const resolvedList =
    statusFilter === "resolved"
      ? (resolvedQuery.data ?? EMPTY_RESOLVED_ISSUES)
      : EMPTY_RESOLVED_ISSUES;
  const activeQuery =
    statusFilter === "resolved" ? resolvedQuery : pausedFilter ? pausedQuery : null;

  return {
    statusFilter,
    setStatusFilter,
    pausedWorkItems,
    resolvedList,
    resourceLoading: activeQuery?.isPending ?? false,
    resourceError: activeQuery?.isError
      ? statusFilter === "resolved"
        ? "Resolved issue history could not load."
        : "Paused issue states could not load."
      : null,
    retryResource: () => void activeQuery?.refetch(),
  };
}
