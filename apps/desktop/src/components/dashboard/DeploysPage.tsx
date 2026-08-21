import { useState, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { InlineIntegrationSetup } from "@/components/settings/InlineIntegrationSetup";
import { SurfaceState } from "@/components/ui/surface-state";
import { AlertCircle, ChevronLeft, ChevronRight, FolderOpen, RotateCcw } from "lucide-react";
import { getScoreClass } from "@/lib/types";
import {
  CommitRow,
  DeployRegressionAlert,
  DeploysLoadingState,
  DeploysPageHeader,
  DeployStatCard,
  GitHubCISection,
} from "./DeploysPageSections";
import type { DeploysPageProps } from "./deploys-page-model";
import { useDeploysPageData } from "./useDeploysPageData";

const COMMITS_PAGE_SIZE = 10;

export function DeploysPage({
  projectPath,
  projectId,
  url,
  onScan,
  scanning,
  onViewScan,
  onAddFolder,
}: DeploysPageProps) {
  const [commitPage, setCommitPage] = useState(1);

  const {
    overview,
    overviewLoading: loading,
    github,
    githubLoading: ghLoading,
    reloadGithub,
  } = useDeploysPageData({ projectId, projectPath, url });
  const gitStatus = overview?.gitStatus ?? null;
  const scanHistory = overview?.scanHistory ?? [];
  const correlations = useMemo(() => overview?.correlations ?? [], [overview?.correlations]);
  const deployEvents = useMemo(() => overview?.deployEvents ?? [], [overview?.deployEvents]);
  const ghData = github?.data ?? null;
  const ghError = github?.failed ?? false;
  const githubConfigured = github?.configured ?? false;

  const hasGit = projectPath && gitStatus?.isGitRepo;
  const latestScan = scanHistory[0] ?? null;
  const commits = useMemo(
    () => (Array.isArray(gitStatus?.commits) ? gitStatus.commits : []),
    [gitStatus],
  );

  // Reset pagination during render so a stale effect cannot override a new click.
  const [prevCommits, setPrevCommits] = useState(commits);
  if (prevCommits !== commits) {
    setPrevCommits(commits);
    setCommitPage(1);
  }

  const totalCommitPages = Math.max(1, Math.ceil(commits.length / COMMITS_PAGE_SIZE));
  const currentCommitPage = Math.min(commitPage, totalCommitPages);
  const commitPageStart = (currentCommitPage - 1) * COMMITS_PAGE_SIZE;
  const commitPageEnd = Math.min(commitPageStart + COMMITS_PAGE_SIZE, commits.length);
  const visibleCommits = commits.slice(commitPageStart, commitPageEnd);
  const commitRangeLabel =
    commits.length > 0
      ? `Showing ${commitPageStart + 1}-${commitPageEnd} of ${commits.length}`
      : "No commits returned for this repository";

  const pendingCommits = useMemo(() => {
    if (!hasGit || !gitStatus || !latestScan) return commits;
    const lastDate = new Date(latestScan.timestamp);
    return commits.filter((c) => new Date(c.date) > lastDate);
  }, [commits, gitStatus, latestScan, hasGit]);

  const successRate = useMemo(() => {
    const runs = Array.isArray(ghData?.workflow_runs) ? ghData.workflow_runs : [];
    if (runs.length === 0) return null;
    const completed = runs.filter((r) => r.conclusion);
    if (!completed.length) return null;
    const passed = completed.filter((r) => r.conclusion === "success").length;
    return ((passed / completed.length) * 100).toFixed(1);
  }, [ghData]);

  const deployRegressions = useMemo(
    () =>
      correlations
        .filter((entry) => entry.correlationType === "deploy_to_regression")
        .sort((a, b) => {
          const aTime = a.targetTimestamp ?? a.sourceTimestamp;
          const bTime = b.targetTimestamp ?? b.sourceTimestamp;
          return bTime.localeCompare(aTime);
        }),
    [correlations],
  );

  const latestDeployRegression = useMemo(() => {
    if (deployRegressions.length === 0) return null;
    if (!latestScan) return deployRegressions[0] ?? null;
    return (
      deployRegressions.find((entry) => entry.targetTimestamp === latestScan.timestamp) ??
      deployRegressions[0] ??
      null
    );
  }, [deployRegressions, latestScan]);

  const latestRegressionDeployEvent = useMemo(
    () =>
      latestDeployRegression
        ? (deployEvents.find((event) => event.id === latestDeployRegression.sourceEventId) ?? null)
        : null,
    [deployEvents, latestDeployRegression],
  );

  const latestRegressionCommitHash = latestRegressionDeployEvent?.sourceId ?? null;

  if (
    loading &&
    !gitStatus &&
    scanHistory.length === 0 &&
    deployEvents.length === 0 &&
    correlations.length === 0
  ) {
    return <DeploysLoadingState onScan={onScan} scanning={scanning} />;
  }

  if (!loading && !hasGit) {
    return (
      <div className="page-content stack-hero">
        <DeploysPageHeader onScan={onScan} scanning={scanning} />
        <SurfaceState
          kind="empty"
          icon={<FolderOpen className="empty-state-icon" />}
          title="No deploy tracking available"
          description={
            projectPath
              ? "No .git directory was found here. Deploy tracking needs a real git repository so SiteCMD can connect commits, scans, and regressions."
              : "Link a local project folder with a git repository to track commits, deploys, and post-deploy regressions."
          }
          primaryAction={!projectPath ? { label: "Add Folder", onClick: onAddFolder } : undefined}
        />
      </div>
    );
  }

  return (
    <div className="page-content stack-hero">
      <DeploysPageHeader onScan={onScan} scanning={scanning} />

      <div className="deploy-stat-grid">
        <DeployStatCard
          label="Total Commits"
          value={String(gitStatus?.totalCommits ?? 0)}
          sub={gitStatus?.branch ? `on ${gitStatus.branch}` : undefined}
        />
        <DeployStatCard
          label="Success Rate"
          value={successRate ? `${successRate}%` : "-"}
          sub={successRate ? "CI pipeline" : "Connect GitHub"}
        />
        <DeployStatCard
          label="Last Web Scan"
          value={latestScan ? String(latestScan.overallScore) : "-"}
          sub={latestScan ? `${latestScan.issuesTotal} issues found` : "No scans yet"}
          scoreColor={latestScan ? getScoreClass(latestScan.overallScore) : undefined}
          onClick={latestScan ? () => onViewScan(latestScan.id) : undefined}
        />
      </div>

      {pendingCommits.length > 0 && scanHistory.length > 0 && (
        <div className="panel panel--compact panel--warning row-between">
          <div className="row-loose">
            <AlertCircle className="icon-lg text-severity-medium" />
            <span className="text-body deploy-pending-text">
              {pendingCommits.length} commit{pendingCommits.length !== 1 ? "s" : ""} since last scan
            </span>
          </div>
          <Button size="sm" onClick={onScan} disabled={scanning} className="btn--gap-tight">
            <RotateCcw className="icon-sm" /> Refresh
          </Button>
        </div>
      )}

      {latestDeployRegression && latestScan ? (
        <DeployRegressionAlert
          correlation={latestDeployRegression}
          latestScan={latestScan}
          onViewScan={() => onViewScan(latestScan.id)}
          onScan={onScan}
          scanning={scanning}
        />
      ) : null}

      {ghData ? (
        <GitHubCISection data={ghData} />
      ) : !ghLoading ? (
        <div className="stack-base">
          {githubConfigured && ghError ? (
            <p className="text-body deploy-reconnect-note">
              GitHub CI stopped syncing. Reconnect to restore pipeline status.
            </p>
          ) : null}
          <InlineIntegrationSetup
            serviceTypes={["github"]}
            projectId={projectId}
            url={url}
            onConnected={() => void reloadGithub()}
            allowReconnect={githubConfigured && ghError ? ["github"] : []}
          />
        </div>
      ) : null}

      <div className="card card--muted card--spacious">
        <div className="row-between deploy-commits-head">
          <div>
            <p className="section-label-mid">Latest Commits</p>
            <p className="subtitle-xs deploy-range-label">{commitRangeLabel}</p>
          </div>
          {totalCommitPages > 1 ? (
            <div className="row">
              <Button
                variant="outline"
                size="sm"
                aria-label="Previous commits page"
                onClick={() => setCommitPage((page) => Math.max(1, page - 1))}
                disabled={currentCommitPage === 1}>
                <ChevronLeft className="icon-sm" />
                Previous
              </Button>
              <span className="subtitle-xs deploy-page-indicator">
                {currentCommitPage}/{totalCommitPages}
              </span>
              <Button
                variant="outline"
                size="sm"
                aria-label="Next commits page"
                onClick={() => setCommitPage((page) => Math.min(totalCommitPages, page + 1))}
                disabled={currentCommitPage === totalCommitPages}>
                Next
                <ChevronRight className="icon-sm" />
              </Button>
            </div>
          ) : null}
        </div>
        <div className="deploy-commits-list">
          {loading
            ? [1, 2, 3, 4, 5].map((i) => (
                <div key={i} className="deploy-skeleton-row">
                  <span className="loading-avatar" />
                  <div className="flex-fill stack-snug">
                    <span className="loading-row-title-wide" />
                    <span className="loading-row-detail-wide" />
                  </div>
                </div>
              ))
            : visibleCommits.map((commit) => {
                const isPending = pendingCommits.some((c) => c.hash === commit.hash);
                const isScanned = !isPending && scanHistory.length > 0;
                return (
                  <CommitRow
                    key={commit.hash}
                    commit={commit}
                    branch={gitStatus?.branch ?? null}
                    isScanned={isScanned}
                    isPending={isPending}
                    isLikelyRegression={latestRegressionCommitHash === commit.hash}
                    regressionConfidence={latestDeployRegression?.confidence ?? null}
                    onScan={onScan}
                    scanning={scanning}
                  />
                );
              })}
          {!loading && visibleCommits.length === 0 ? (
            <p className="text-body-muted">No commits are available for this repository yet.</p>
          ) : null}
        </div>
      </div>
    </div>
  );
}
