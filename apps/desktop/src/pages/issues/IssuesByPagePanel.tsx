import { useMemo } from "react";
import { ArrowLeft, ChevronRight, Globe2 } from "lucide-react";
import { ByPageList } from "@/components/issues/ByPageList";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import type { IssueGroup, ScanCategory } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { SurfaceState } from "@/components/ui/surface-state";
import { compareSeverity, formatSeverityLabel, severityToneClass } from "@/lib/severity";
import { CATEGORY_LABELS, formatCheckName } from "@/lib/tokens";
import { formatUrlHost, formatUrlPathOrHost, getUrlPathname } from "@/lib/utils";
import { IssuePanelSkeleton } from "@/components/issues/IssuePanelSkeleton";

export function IssuesByPagePanel({
  projectId,
  url,
  selectedPageUrl,
  pageGroups,
  pageGroupsLoading,
  pageGroupsError,
  onRetryPageGroups,
  onSelectPage,
  onSelectIssue,
}: {
  projectId: number;
  url: string;
  selectedPageUrl: string | null;
  pageGroups: IssueGroup[];
  pageGroupsLoading: boolean;
  pageGroupsError: string | null;
  onRetryPageGroups: () => void;
  onSelectPage: (url: string | null) => void;
  onSelectIssue: (checkId: string) => void;
}) {
  const visibleGroups = useMemo(
    () =>
      pageGroups
        .filter((group) => !["blocked", "ignored", "snoozed", "verified"].includes(group.status))
        .sort((left, right) => {
          const severityDelta = compareSeverity(left.severity, right.severity);
          if (severityDelta !== 0) return severityDelta;
          return left.title.localeCompare(right.title);
        }),
    [pageGroups],
  );

  if (!selectedPageUrl) {
    return (
      <ByPageList
        projectId={projectId}
        envUrl={normalizeAppUrlForKey(url)}
        onSelectPage={onSelectPage}
      />
    );
  }

  const pathLabel = getUrlPathname(
    selectedPageUrl,
    formatUrlPathOrHost(selectedPageUrl, "Selected page"),
  );
  const hostLabel = formatUrlHost(selectedPageUrl, selectedPageUrl);
  const issueCountLabel = pageGroupsLoading
    ? "Loading issues"
    : pageGroupsError
      ? "Count unavailable"
      : `${visibleGroups.length} open issue${visibleGroups.length === 1 ? "" : "s"}`;

  return (
    <div>
      <div className="by-page-detail-header">
        <Button
          unstyled
          type="button"
          className="inline-back-button"
          onClick={() => onSelectPage(null)}>
          <ArrowLeft className="icon-sm" aria-hidden="true" />
          All pages
        </Button>
        <div className="by-page-detail-header__context">
          <span className="by-page-detail-header__icon" aria-hidden="true">
            <Globe2 className="icon-md" />
          </span>
          <div className="by-page-detail-header__identity">
            <p className="by-page-detail-header__path">{pathLabel}</p>
            <p className="by-page-detail-header__host">{hostLabel}</p>
          </div>
        </div>
        <span className="by-page-detail-header__count">{issueCountLabel}</span>
      </div>
      {pageGroupsLoading ? (
        <IssuePanelSkeleton label="Loading page issues" />
      ) : pageGroupsError ? (
        <SurfaceState
          kind="error"
          title="Page issues could not load"
          description={`${pageGroupsError} Retry before treating this page as clear.`}
          className="panel-inset"
          primaryAction={{ label: "Retry", onClick: onRetryPageGroups }}
        />
      ) : visibleGroups.length === 0 ? (
        <SurfaceState
          kind="empty"
          title="No open issues on this page"
          description="The page no longer has an active finding attached to it. Return to the page list to review the remaining URLs."
          className="panel-inset"
          primaryAction={{ label: "Back to pages", onClick: () => onSelectPage(null) }}
        />
      ) : (
        <div>
          <div className="by-page-detail-intro">
            <p className="by-page-detail-intro__title">Findings on this page</p>
            <p className="by-page-detail-intro__description">
              Ordered by severity. Select a finding to open its full issue dossier.
            </p>
          </div>
          <div className="by-page-issue-list">
            {visibleGroups.map((group) => {
              const categoryLabel =
                CATEGORY_LABELS[group.category as ScanCategory] ?? formatCheckName(group.category);
              const description = formatIssueDescription(group.description);

              return (
                <Button
                  unstyled
                  type="button"
                  key={group.checkId}
                  className="by-page-issue-row"
                  aria-label={`Open ${group.title}`}
                  onClick={() => onSelectIssue(group.checkId)}>
                  <span className="by-page-issue-row__content">
                    <span className="by-page-issue-row__meta">
                      <span className={severityToneClass(group.severity)}>
                        {formatSeverityLabel(group.severity)}
                      </span>
                      <span aria-hidden="true">·</span>
                      <span>{categoryLabel}</span>
                    </span>
                    <span className="by-page-issue-row__title">{group.title}</span>
                    <span className="by-page-issue-row__description">{description}</span>
                  </span>
                  <ChevronRight className="list-row__chevron icon-md" aria-hidden="true" />
                </Button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

function formatIssueDescription(value: string): string {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (!normalized) return "Open the dossier to review the finding and its recommended next step.";
  return normalized.length > 220 ? `${normalized.slice(0, 217).trimEnd()}...` : normalized;
}
