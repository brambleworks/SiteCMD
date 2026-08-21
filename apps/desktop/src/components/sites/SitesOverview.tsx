import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { InlineSkeleton, LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { AlertTriangle, ArrowRight, Globe, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SurfaceState } from "@/components/ui/surface-state";
import { PageGuideButton } from "@/components/layout/PageGuide";
import {
  getAllProjectsWorkSummary,
  type TodayProjectWorkSummary,
} from "@/lib/project-summary-signals";
import { getScoreClass } from "@/lib/score";
import { formatUrlHost } from "@/lib/utils";
import { queryKeys } from "@/lib/query/query-keys";

interface SitesOverviewProps {
  onSelectProject: (projectId: number) => void;
  onAddProject: () => void;
  currentProjectId?: number;
}

export function SitesOverview({
  onSelectProject,
  onAddProject,
  currentProjectId,
}: SitesOverviewProps) {
  const projectsQuery = useQuery<TodayProjectWorkSummary[]>({
    queryKey: queryKeys.sites.overview(),
    queryFn: () => getAllProjectsWorkSummary(),
  });
  const projects = useMemo(() => projectsQuery.data ?? [], [projectsQuery.data]);
  const loadError = projectsQuery.isError ? "Sites could not load right now." : null;

  // Keep the connected multi-site preview consistent across every mount point.

  const aggregates = useMemo(() => {
    if (!projects || projects.length === 0) return null;
    const totalIssues = projects.reduce((sum, p) => sum + p.siteIssueCount, 0);
    const scoredProjects = projects
      .map((project) => projectSiteScore(project))
      .filter((score): score is number => score != null);
    const avgSiteScore =
      scoredProjects.length > 0
        ? Math.round(scoredProjects.reduce((sum, score) => sum + score, 0) / scoredProjects.length)
        : null;
    return { totalIssues, avgSiteScore, scannedSites: scoredProjects.length };
  }, [projects]);

  return (
    <div className="page-content stack-section">
      <div className="sites-header">
        <div>
          <h1 className="sites-title">Overview</h1>
          <p className="text-13-muted sites-subtitle">
            All tracked sites, their current workload, and the fastest place to jump next.
          </p>
        </div>
        <div className="row no-shrink">
          <PageGuideButton page="sites" />
          <Button size="sm" className="sites-add-btn" onClick={onAddProject}>
            <Plus className="icon-xs" /> Add Site
          </Button>
        </div>
      </div>

      {projects.length > 0 && aggregates && (
        <div className="sites-stat-grid">
          <div className="stat-card">
            <span className="stat-label">Total Sites</span>
            <span className="stat-value text-foreground">{projects.length}</span>
          </div>
          <div className="stat-card">
            <span className="stat-label">Active Issues</span>
            <span
              className={`stat-value ${aggregates.totalIssues > 0 ? "text-severity-critical" : "text-foreground"}`}>
              {aggregates.totalIssues}
            </span>
          </div>
          <div
            className="stat-card"
            title="The mean of each scanned site's live SiteCMD health score. It is an average, not the worst site, and it covers only sites that have been scanned.">
            <span className="stat-label">Avg. SiteCMD Score</span>
            {aggregates.avgSiteScore != null ? (
              <span className={`stat-value ${scoreClass(aggregates.avgSiteScore)}`}>
                {aggregates.avgSiteScore}
              </span>
            ) : (
              <span className="stat-value text-muted-foreground">--</span>
            )}
          </div>
          <div className="stat-card">
            <span className="stat-label">Scanned Sites</span>
            <span className="stat-value text-foreground">{aggregates.scannedSites}</span>
          </div>
        </div>
      )}

      {projects.length > 0 && aggregates && (
        <p className="text-13-muted sites-avg-note">
          Each Score is that site's live SiteCMD health score. The average above covers the{" "}
          {aggregates.scannedSites} scanned site{aggregates.scannedSites === 1 ? "" : "s"}; sites
          with no score yet are left out rather than counted as zero.
        </p>
      )}

      {projectsQuery.isPending ? (
        <LoadingRegion label="Overview loading state" className="stack-card">
          <div className="sites-stat-grid">
            {[0, 1, 2, 3].map((i) => (
              <div key={i} className="stat-card">
                <Skeleton className="sites-sk-stat-label" />
                <Skeleton className="sites-sk-stat-value" />
              </div>
            ))}
          </div>

          <div className="stack-base">
            {[1, 2, 3].map((i) => (
              <div key={i} className="overview-site-row bg-card">
                <div className="overview-site-main">
                  <InlineSkeleton className="sites-sk-icon" />
                  <div className="flex-fill stack-snug">
                    <InlineSkeleton className="sites-sk-name" />
                    <InlineSkeleton className="sites-sk-url" />
                  </div>
                </div>

                <div className="overview-issues-badge">
                  <InlineSkeleton className="sites-sk-issues" />
                </div>

                <div className="overview-score-group">
                  <div className="overview-score-col">
                    <Skeleton className="sites-sk-score-label" />
                    <Skeleton className="sites-sk-score" />
                  </div>
                  <div className="overview-score-col">
                    <Skeleton className="sites-sk-score-label-wide" />
                    <Skeleton className="sites-sk-score" />
                  </div>
                  <Skeleton className="sites-sk-arrow" />
                </div>
              </div>
            ))}
          </div>
        </LoadingRegion>
      ) : loadError ? (
        <SurfaceState
          kind="error"
          title="Sites could not load"
          description="We could not refresh your project list right now. Try again in a moment."
          primaryAction={{ label: "Retry", onClick: () => void projectsQuery.refetch() }}
        />
      ) : projects.length === 0 ? (
        <SurfaceState
          kind="empty"
          title="No projects yet"
          description="Add your first site to start tracking health scores and issues."
          primaryAction={{ label: "Add your first site", onClick: onAddProject }}
        />
      ) : (
        <div className="stack-base">
          {projects.map((p) => (
            <SiteRow
              key={p.id}
              project={p}
              isActive={p.id === currentProjectId}
              onSelect={onSelectProject}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function SiteRow({
  project: p,
  isActive,
  onSelect,
}: {
  project: TodayProjectWorkSummary;
  isActive: boolean;
  onSelect: (id: number) => void;
}) {
  const totalIssues = p.siteIssueCount;
  const hasCritical = p.siteCriticalCount > 0;
  const siteScore = projectSiteScore(p);

  return (
    <Button unstyled onClick={() => onSelect(p.id)} className="overview-site-row">
      <div className="overview-site-main">
        <div className="overview-site-icon">
          <Globe className="icon-md text-muted-foreground" />
        </div>
        <div className="min-w-0">
          <div className="row">
            <span className="overview-site-name text-truncate">{p.name}</span>
            {isActive && <span className="overview-current-label">current</span>}
          </div>
          <span className="text-body-muted text-truncate overview-site-url">
            {formatUrlHost(p.primaryUrl, p.primaryUrl)}
          </span>
        </div>
      </div>

      {totalIssues > 0 && (
        <span
          className={`overview-issues-badge text-body-muted ${
            hasCritical ? "text-severity-critical" : "text-severity-medium"
          }`}>
          <AlertTriangle className="icon-xs" />
          {totalIssues} issue{totalIssues !== 1 ? "s" : ""}
        </span>
      )}

      <div className="overview-score-group">
        <div
          className="overview-score-col"
          title="This site's live SiteCMD health score - the same number its dashboard headlines.">
          <span className="stat-label">Score</span>
          {siteScore != null ? (
            <span className={`overview-score-value ${scoreClass(siteScore)}`}>{siteScore}</span>
          ) : (
            <span className="text-body text-muted-foreground">--</span>
          )}
        </div>

        <ArrowRight className="icon-sm text-muted-foreground overview-arrow" />
      </div>
    </Button>
  );
}

// Score classes come from `@/lib/score` (single source of truth).
// Local alias kept for the four call sites below; no behavior of its own.
const scoreClass = getScoreClass;

function projectSiteScore(project: TodayProjectWorkSummary): number | null {
  return project.siteScore;
}
