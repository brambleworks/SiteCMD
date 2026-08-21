import { HeaderActions } from "@/app/ShellHeader";
import { Button } from "@/components/ui/button";
import { ExtLink } from "@/components/ui/external-link";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { formatRelativeTime } from "@/lib/tokens";
import { useCurrentTime } from "@/lib/useCurrentTime";
import type { GitHubData, PullRequest, WorkflowRun } from "@/lib/analytics-types";
import {
  AlertCircle,
  CheckCircle2,
  Clock,
  ExternalLink as ExternalLinkIcon,
  GitCommit as GitCommitIcon,
  GitPullRequest,
  Play,
  RotateCcw,
  XCircle,
} from "lucide-react";
import type { DeployCorrelation, DeployScanSummary, GitCommit } from "./deploys-page-model";

export function DeploysLoadingState({
  onScan,
  scanning,
}: {
  onScan: () => void;
  scanning: boolean;
}) {
  return (
    <LoadingRegion label="Deploys loading state" className="page-content stack-hero">
      <DeploysPageHeader onScan={onScan} scanning={scanning} />

      <div className="deploy-stat-grid">
        {[0, 1, 2].map((index) => (
          <div key={index} className="card card--muted card--spacious">
            <Skeleton className="deploy-sk-label" />
            <Skeleton className="deploy-sk-value" />
            <Skeleton className="deploy-sk-sub" />
          </div>
        ))}
      </div>

      <div className="card card--muted card--spacious">
        <div className="row-between">
          <div className="stack-snug">
            <Skeleton className="deploy-sk-line" />
            <Skeleton className="deploy-sk-line-wide" />
          </div>
          <Skeleton className="deploy-sk-btn" />
        </div>
      </div>

      <div className="card card--muted card--spacious">
        <div className="row deploy-section-head">
          <Skeleton className="deploy-sk-ci-a" />
          <Skeleton className="deploy-sk-ci-b" />
        </div>
        <div className="stack-base">
          {[0, 1, 2, 3, 4].map((index) => (
            <div key={index} className="row-between commit-row ghost-border">
              <div className="commit-row-main">
                <Skeleton className="deploy-sk-avatar" />
                <div className="flex-fill stack-snug">
                  <div className="row-loose">
                    <Skeleton className="deploy-sk-msg" />
                    <Skeleton className="deploy-sk-badge" />
                  </div>
                  <div className="row">
                    <Skeleton className="deploy-sk-meta" />
                    <Skeleton className="deploy-sk-dot" />
                    <Skeleton className="deploy-sk-meta-wide" />
                    <Skeleton className="deploy-sk-dot" />
                    <Skeleton className="deploy-sk-meta" />
                  </div>
                </div>
              </div>
              <Skeleton className="deploy-sk-row-btn" />
            </div>
          ))}
        </div>
      </div>
    </LoadingRegion>
  );
}

export function DeploysPageHeader({ onScan, scanning }: { onScan: () => void; scanning: boolean }) {
  return (
    <HeaderActions>
      <Button onClick={onScan} disabled={scanning} className="btn--gap-snug">
        <RotateCcw className="icon-md" />
        {scanning ? "Refreshing..." : "Refresh"}
      </Button>
    </HeaderActions>
  );
}

export function DeployStatCard({
  label,
  value,
  sub,
  scoreColor,
  onClick,
}: {
  label: string;
  value: string;
  sub?: string;
  scoreColor?: string;
  onClick?: () => void;
}) {
  return (
    <div
      className={`card card--muted card--spacious metric-card ${onClick ? "card--interactive" : ""}`}
      onClick={onClick}>
      <p className="section-label-mid deploy-stat-label">{label}</p>
      <span className={`deploy-stat-value ${scoreColor ?? "text-foreground"}`}>{value}</span>
      {sub && <p className="subtitle-xs deploy-range-label">{sub}</p>}
    </div>
  );
}

export function CommitRow({
  commit,
  branch,
  isScanned,
  isPending,
  isLikelyRegression,
  regressionConfidence,
  onScan,
  scanning,
}: {
  commit: GitCommit;
  branch: string | null;
  isScanned: boolean;
  isPending: boolean;
  isLikelyRegression?: boolean;
  regressionConfidence?: DeployCorrelation["confidence"] | null;
  onScan: () => void;
  scanning: boolean;
}) {
  const toneLabel = isLikelyRegression
    ? regressionConfidence === "high"
      ? "Likely regression"
      : "Possible regression"
    : null;

  return (
    <div className="row-between commit-row commit-row--hover ghost-border">
      <div className="commit-row-main">
        <div className="commit-status">
          {isScanned ? (
            <CheckCircle2 className="icon-lg text-score-excellent" />
          ) : isPending ? (
            <Clock className="icon-lg text-severity-medium" />
          ) : (
            <GitCommitIcon className="icon-lg text-muted-foreground" />
          )}
        </div>
        <div className="min-w-0">
          <div className="row-loose">
            <h3 className="row-title-lg text-truncate commit-message">{commit.message}</h3>
            {toneLabel ? <span className="text-meta commit-regression">{toneLabel}</span> : null}
            {branch && <span className="text-mono-xs text-primary commit-branch">{branch}</span>}
          </div>
          <div className="row commit-meta">
            <span className="subtitle-xs">{commit.author}</span>
            <span className="text-muted-foreground commit-dot-sep">•</span>
            <span className="subtitle-xs">{commit.relativeDate}</span>
            <span className="text-muted-foreground commit-dot-sep">•</span>
            <span className="mono-subtle">{commit.shortHash}</span>
          </div>
        </div>
      </div>
      {isPending && (
        <Button
          variant="outline"
          size="sm"
          onClick={(e) => {
            e.stopPropagation();
            onScan();
          }}
          disabled={scanning}
          className="commit-scan-btn">
          Scan after deploy
        </Button>
      )}
    </div>
  );
}

export function DeployRegressionAlert({
  correlation,
  latestScan,
  onViewScan,
  onScan,
  scanning,
}: {
  correlation: DeployCorrelation;
  latestScan: DeployScanSummary;
  onViewScan: () => void;
  onScan: () => void;
  scanning: boolean;
}) {
  const highConfidence = correlation.confidence === "high";
  return (
    <div
      className={`panel panel--spacious stack-card ${highConfidence ? "panel--danger" : "panel--warning"}`}>
      <div className="row-start">
        <AlertCircle
          className={`icon-lg ${highConfidence ? "text-severity-critical" : "text-severity-medium"}`}
        />
        <div className="min-w-0">
          <p
            className={`regression-label ${highConfidence ? "text-severity-critical" : "text-severity-medium"}`}>
            {highConfidence ? "Deploy Likely Caused Regression" : "Deploy May Explain Score Drop"}
          </p>
          <p className="regression-desc">{correlation.description}</p>
          <p className="text-body-muted text-relaxed regression-body">
            Latest Web Scan is currently{" "}
            <span className="text-foreground regression-strong">{latestScan.overallScore}/100</span>{" "}
            with <span className="text-foreground regression-strong">{latestScan.issuesTotal}</span>{" "}
            open issue{latestScan.issuesTotal === 1 ? "" : "s"}. Review the release, then rerun Web
            Scan to make sure your scan data matches what actually shipped.
          </p>
        </div>
      </div>

      <div className="regression-actions">
        <Button size="sm" className="btn--gap-tight" onClick={onViewScan}>
          Open dropped scan
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="btn--gap-tight"
          onClick={onScan}
          disabled={scanning}>
          <RotateCcw className="icon-sm" />
          {scanning ? "Refreshing…" : "Scan after deploy"}
        </Button>
      </div>
    </div>
  );
}

export function GitHubCISection({ data }: { data: GitHubData }) {
  const nowMs = useCurrentTime();
  const recentRuns = data.workflow_runs.slice(0, 5);
  const openPrs = data.open_prs;

  if (!recentRuns.length && !openPrs.length && !data.deployments.length) return null;

  return (
    <div className="stack-card">
      {recentRuns.length > 0 && (
        <div className="card card--muted card--spacious">
          <div className="row-between deploy-section-head">
            <span className="eyebrow--alt text-muted-foreground">CI / Actions</span>
            <span className="subtitle-xs text-mono">{data.repo}</span>
          </div>
          <div className="stack-snug">
            {recentRuns.map((run) => (
              <CIRunRow key={run.id} run={run} nowMs={nowMs} />
            ))}
          </div>
        </div>
      )}

      {openPrs.length > 0 && (
        <div className="card card--muted card--spacious">
          <span className="eyebrow--alt text-muted-foreground">Open Pull Requests</span>
          <div className="stack-snug ci-pr-list">
            {openPrs.map((pr) => (
              <PRRow key={pr.number} pr={pr} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function CIRunRow({ run, nowMs }: { run: WorkflowRun; nowMs: number }) {
  const icon =
    run.conclusion === "success" ? (
      <CheckCircle2 className="icon-md text-score-excellent" />
    ) : run.conclusion === "failure" ? (
      <XCircle className="icon-md text-severity-critical" />
    ) : run.status === "in_progress" ? (
      <Play className="icon-md text-primary" />
    ) : (
      <Clock className="icon-muted no-shrink" />
    );

  const duration = run.duration_seconds
    ? run.duration_seconds < 60
      ? `${run.duration_seconds}s`
      : `${Math.round(run.duration_seconds / 60)}m`
    : null;

  return (
    <div className="ci-row">
      {icon}
      <span className="ci-row-title text-truncate flex-fill">{run.name}</span>
      <span className="subtitle-xs text-mono">{run.head_branch}</span>
      {duration && <span className="subtitle-xs">{duration}</span>}
      <span className="subtitle-xs">{formatRelativeTime(new Date(run.created_at), nowMs)}</span>
      <ExtLink href={run.html_url} className="text-hover">
        <ExternalLinkIcon className="icon-sm" />
      </ExtLink>
    </div>
  );
}

function PRRow({ pr }: { pr: PullRequest }) {
  return (
    <div className="ci-row">
      <GitPullRequest
        className={`icon-md ${pr.draft ? "text-muted-foreground" : "text-score-excellent"}`}
      />
      <span className="ci-row-title text-truncate flex-fill">
        #{pr.number} {pr.title}
      </span>
      <span className="subtitle-xs">{pr.user}</span>
      <span className="subtitle-xs">
        +{pr.additions} −{pr.deletions}
      </span>
      <ExtLink href={pr.html_url} className="text-hover">
        <ExternalLinkIcon className="icon-sm" />
      </ExtLink>
    </div>
  );
}
