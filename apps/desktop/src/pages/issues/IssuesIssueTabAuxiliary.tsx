import type { IssueStatusFilter } from "@/components/issues/IssueList";
import type { ProjectWorkItem } from "@/lib/project-summary-types";
import type { ResolvedIssue } from "@/lib/resolved-issues";
import { Button } from "@/components/ui/button";
import { Loader2, RotateCcw } from "lucide-react";

interface PausedIssuesListProps {
  onRestore: (checkId: string) => void | Promise<void>;
  pausedWorkItems: ProjectWorkItem[];
  restoringCheckId: string | null;
  statusFilter: IssueStatusFilter;
}

export function PausedIssuesList({
  onRestore,
  pausedWorkItems,
  restoringCheckId,
  statusFilter,
}: PausedIssuesListProps) {
  if (statusFilter !== "ignored" && statusFilter !== "blocked") {
    return null;
  }

  return (
    <div className="aux-issue-list">
      {pausedWorkItems.map((item) => (
        <div key={item.stableKey} className="aux-issue-row">
          <div className="flex-fill">
            <p className="text-body text-truncate">{item.title}</p>
            <p className="text-micro aux-issue-meta">
              {item.status} · {item.category ?? item.domain ?? item.kind}
            </p>
            {item.summary ? (
              <p className="text-meta text-truncate aux-issue-summary">{item.summary}</p>
            ) : null}
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-label={`Restore ${item.title}`}
            onClick={() => void onRestore(item.stableKey)}
            disabled={restoringCheckId != null}>
            {restoringCheckId === item.stableKey ? (
              <Loader2 className="spinner-sm" />
            ) : (
              <RotateCcw className="icon-sm" />
            )}
            Restore
          </Button>
        </div>
      ))}
      {pausedWorkItems.length === 0 ? (
        <p className="aux-issue-empty">No {statusFilter} issues.</p>
      ) : null}
    </div>
  );
}

interface ResolvedIssuesListProps {
  resolvedList: ResolvedIssue[];
  statusFilter: IssueStatusFilter;
}

export function ResolvedIssuesList({ resolvedList, statusFilter }: ResolvedIssuesListProps) {
  if (statusFilter !== "resolved") return null;

  return (
    <div className="aux-issue-list">
      {resolvedList.map((item) => (
        <div
          key={`${item.checkId}-${item.resolvedScanId ?? item.resolvedAt}-${item.recurrenceCount}`}
          className="aux-issue-row">
          <div className="flex-fill">
            <p className="text-body text-truncate">{item.title}</p>
            <p className="text-micro aux-issue-meta">
              Resolved {new Date(item.resolvedAt).toLocaleDateString()}
              {item.recurrenceCount > 1 ? ` · ${item.recurrenceCount}x recurrence` : ""}
            </p>
          </div>
        </div>
      ))}
      {resolvedList.length === 0 ? (
        <p className="aux-issue-empty">
          Nothing resolved yet. Fix an issue and it will appear here after the next scan.
        </p>
      ) : null}
    </div>
  );
}
