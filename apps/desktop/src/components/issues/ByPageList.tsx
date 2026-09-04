import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ChevronRight, Globe2 } from "lucide-react";
import type { PageSummary } from "@/lib/types";
import { getIssuePages } from "@/lib/issues";
import { Button } from "@/components/ui/button";
import { SurfaceState } from "@/components/ui/surface-state";
import { Pager } from "@/components/ui/pager";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { pageWindow } from "@/lib/pagination";
import { formatSeverityLabel, severityToneClass } from "@/lib/severity";
import { formatUrlHost, formatUrlPathOrHost, getUrlPathname } from "@/lib/utils";
import { queryKeys } from "@/lib/query/query-keys";
import { IssuePanelSkeleton } from "@/components/issues/IssuePanelSkeleton";

interface Props {
  projectId: number;
  envUrl: string;
  onSelectPage: (pageUrl: string) => void;
}

const PAGE_SIZE = 50;

export function ByPageList({ projectId, envUrl, onSelectPage }: Props) {
  const [page, setPage] = useState(1);
  useResetOnChange(`${projectId}:${envUrl}`, () => setPage(1));
  const pagesQuery = useQuery<PageSummary[]>({
    queryKey: queryKeys.issuePages.forEnv(projectId, envUrl),
    queryFn: () => getIssuePages(projectId, envUrl),
  });
  const visiblePages = useMemo(
    () =>
      (pagesQuery.data ?? []).filter(
        (row) => row.pageUrl !== "__project_wide__" && row.pageUrl.trim().length > 0,
      ),
    [pagesQuery.data],
  );
  const bounded = pageWindow(visiblePages, page, PAGE_SIZE);

  if (pagesQuery.isPending) {
    return <IssuePanelSkeleton label="Loading affected pages" />;
  }
  if (pagesQuery.isError) {
    return (
      <SurfaceState
        kind="error"
        title="Affected pages could not load"
        description="SiteCMD could not read the page-level issue index. Retry before treating the page list as clear."
        className="panel-inset"
        primaryAction={{ label: "Retry", onClick: () => void pagesQuery.refetch() }}
      />
    );
  }
  if (visiblePages.length === 0) {
    return (
      <SurfaceState
        kind="empty"
        title="No page-specific issues"
        description="Only findings tied to a specific URL appear here. Code findings remain in Issues, while dependency updates stay in Updates."
        className="panel-inset"
        primaryAction={{ label: "Refresh", onClick: () => void pagesQuery.refetch() }}
      />
    );
  }

  return (
    <div>
      <div className="by-page-overview">
        <div className="by-page-overview__copy">
          <p className="by-page-overview__title">Pages with open findings</p>
          <p className="by-page-overview__description">
            Review issues in the context of the URL they affect. Code findings remain in Issues,
            while dependency updates stay in Updates.
          </p>
        </div>
        <span className="by-page-overview__count">
          {visiblePages.length} affected page{visiblePages.length === 1 ? "" : "s"}
        </span>
      </div>

      <div className="by-page-list">
        {bounded.rows.map((page) => {
          const pathLabel = getUrlPathname(
            page.pageUrl,
            formatUrlPathOrHost(page.pageUrl, page.label),
          );
          const hostLabel = formatUrlHost(page.pageUrl, page.label);
          const severityLabel = formatSeverityLabel(page.maxSeverity);
          const issueLabel = `${page.issueCount} open issue${page.issueCount === 1 ? "" : "s"}`;

          return (
            <Button
              unstyled
              key={page.pageUrl}
              type="button"
              className="by-page-row"
              aria-label={`Open ${pathLabel} on ${hostLabel}: ${issueLabel}`}
              onClick={() => onSelectPage(page.pageUrl)}>
              <span className="by-page-row__icon" aria-hidden="true">
                <Globe2 className="icon-md" />
              </span>
              <span className="by-page-row__identity">
                <span className="by-page-row__path">{pathLabel}</span>
                <span className="by-page-row__host">{hostLabel}</span>
              </span>
              <span className="by-page-row__summary">
                <span className={`by-page-row__severity ${severityToneClass(page.maxSeverity)}`}>
                  Highest: {severityLabel}
                </span>
                <span className="by-page-row__issue-count">{issueLabel}</span>
                <ChevronRight className="list-row__chevron icon-md" aria-hidden="true" />
              </span>
            </Button>
          );
        })}
      </div>
      {bounded.totalPages > 1 ? (
        <div className="panel-inset stack-base">
          <p className="text-meta" role="status">
            Showing {(bounded.page - 1) * PAGE_SIZE + 1}-
            {Math.min(bounded.page * PAGE_SIZE, visiblePages.length)} of {visiblePages.length}{" "}
            affected pages
          </p>
          <Pager
            page={bounded.page}
            totalPages={bounded.totalPages}
            onChange={setPage}
            label="Affected page results"
            itemLabel="results"
          />
        </div>
      ) : null}
    </div>
  );
}
