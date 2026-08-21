import type { IssueStatusFilter, UnifiedFixIssue } from "@/components/issues/IssueList";
import { IssueList } from "@/components/issues/IssueList";
import type { ScanConfigPreset } from "@/components/scan/ScanConfigOverlay";
import { SurfaceState } from "@/components/ui/surface-state";
import type { ProjectIssueSummary } from "@/lib/project-issue-summary";
import type { ProjectWorkItem } from "@/lib/project-summary-types";
import type { ResolvedIssue } from "@/lib/resolved-issues";
import type { IssueLink } from "@/lib/types";
import { PausedIssuesList, ResolvedIssuesList } from "@/pages/issues/IssuesIssueTabAuxiliary";
import { IssuePanelSkeleton } from "@/components/issues/IssuePanelSkeleton";

// Stable empty inputs for non-queue status views so IssueList's memoized
// derivations keep their identities across renders.
const NO_RANKED_ISSUES: UnifiedFixIssue[] = [];
interface IssuesQueuePanelProps {
  detectedStack?: Record<string, unknown> | null;
  rankedIssues: UnifiedFixIssue[];
  initialFocus?: string | null;
  issueLinks: IssueLink[];
  issueSummary: ProjectIssueSummary;
  onClearSelection: () => void;
  onOpenScanConfig: (preset?: ScanConfigPreset) => void;
  onRefreshDashboard: () => void | Promise<unknown>;
  onRestorePausedIssue: (checkId: string) => void | Promise<void>;
  onSelect: (item: UnifiedFixIssue) => void;
  onStatusChange: (next: IssueStatusFilter) => void;
  pausedWorkItems: ProjectWorkItem[];
  projectPath: string | null;
  resolvedList: ResolvedIssue[];
  restoringPausedCheckId: string | null;
  selectedIssueId: string | null;
  showFirstScanEmpty: boolean;
  showInitialIssuesLoading: boolean;
  showIssuesFailure: boolean;
  statusFilter: IssueStatusFilter;
  statusResourceError: string | null;
  statusResourceLoading: boolean;
  onRetryStatusResource: () => void;
  url: string;
}

export function IssuesQueuePanel({
  detectedStack,
  rankedIssues,
  initialFocus,
  issueLinks,
  issueSummary,
  onClearSelection,
  onOpenScanConfig,
  onRefreshDashboard,
  onRestorePausedIssue,
  onSelect,
  onStatusChange,
  pausedWorkItems,
  projectPath,
  resolvedList,
  restoringPausedCheckId,
  selectedIssueId,
  showFirstScanEmpty,
  showInitialIssuesLoading,
  showIssuesFailure,
  statusFilter,
  statusResourceError,
  statusResourceLoading,
  onRetryStatusResource,
  url,
}: IssuesQueuePanelProps) {
  if (showIssuesFailure) {
    return (
      <SurfaceState
        kind="error"
        title="Issues could not load"
        description="We could not pull the latest issue view for this project. Retry in a moment and SiteCMD will rebuild the list."
        className="panel-inset"
        primaryAction={{ label: "Retry", onClick: onRefreshDashboard }}
      />
    );
  }

  if (showFirstScanEmpty) {
    return (
      <SurfaceState
        kind="empty"
        title="No scans yet"
        description="Run your first scan and this page will turn into the action center for what to fix, what changed, and what to verify next."
        className="panel-inset"
        primaryAction={{ label: "Run Web Scan", onClick: () => onOpenScanConfig() }}
        secondaryAction={
          projectPath
            ? {
                label: "Run Code Scan",
                onClick: () => onOpenScanConfig({ scanType: "code" }),
                variant: "outline",
              }
            : undefined
        }
      />
    );
  }

  const showsStatusResource =
    statusFilter === "ignored" || statusFilter === "blocked" || statusFilter === "resolved";
  if (showsStatusResource && statusResourceLoading) {
    return <IssuePanelSkeleton label="Loading issue history" />;
  }
  if (showsStatusResource && statusResourceError) {
    return (
      <SurfaceState
        kind="error"
        title="Issue history could not load"
        description={`${statusResourceError} Retry before treating this view as empty.`}
        className="panel-inset"
        primaryAction={{ label: "Retry", onClick: onRetryStatusResource }}
      />
    );
  }

  const showsScanQueue = statusFilter === "active" || statusFilter === "all";

  return (
    <>
      <IssueList
        rankedIssues={showsScanQueue ? rankedIssues : NO_RANKED_ISSUES}
        loading={showInitialIssuesLoading}
        issueLinks={issueLinks}
        issueSummary={issueSummary}
        selectedId={selectedIssueId}
        focus={initialFocus}
        onSelect={onSelect}
        onClearSelection={onClearSelection}
        url={url}
        detectedStack={detectedStack}
        statusFilter={statusFilter}
        onStatusChange={onStatusChange}
      />
      <PausedIssuesList
        onRestore={onRestorePausedIssue}
        pausedWorkItems={pausedWorkItems}
        restoringCheckId={restoringPausedCheckId}
        statusFilter={statusFilter}
      />
      <ResolvedIssuesList resolvedList={resolvedList} statusFilter={statusFilter} />
    </>
  );
}
