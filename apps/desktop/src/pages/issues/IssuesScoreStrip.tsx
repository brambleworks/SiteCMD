import { ScoreRing } from "@/components/ui/score-ring";
import type { ProjectIssueSummary } from "@/lib/project-issue-summary";
import { formatRelativeTime } from "@/lib/tokens";
import type { SiteCmdScoreModel } from "@/lib/sitecmd-score";
import { useCurrentTime } from "@/lib/useCurrentTime";

interface IssuesScoreStripProps {
  score: SiteCmdScoreModel | null;
  checkedAt: string | null;
  issueSummary?: Pick<ProjectIssueSummary, "totalCount" | "severityCounts"> | null;
}

function formatCheckedAt(value: string | null, nowMs: number) {
  if (!value) return "Not run yet";
  return `Checked ${formatRelativeTime(new Date(value), nowMs)}`;
}

function issueLabel(count: number) {
  return `${count} issue${count === 1 ? "" : "s"}`;
}

function buildDetail(
  score: SiteCmdScoreModel,
  issueSummary: Pick<ProjectIssueSummary, "totalCount" | "severityCounts"> | null = null,
): string {
  const issueCount = issueSummary?.totalCount ?? score.totalIssues;
  const criticalCount = issueSummary?.severityCounts.critical ?? score.severityTotals.critical;
  const parts = [issueLabel(issueCount)];
  if (criticalCount > 0) parts.push(`${criticalCount} critical`);
  return parts.join(" · ");
}

export function IssuesScoreStrip({ score, checkedAt, issueSummary = null }: IssuesScoreStripProps) {
  const nowMs = useCurrentTime();
  // One note at a time, strongest first: the cap and the floor both outrank the
  // open-issue ceiling, which only ever explains a headline just short of 100.
  const scoreNote =
    score?.breakdown.capNote ?? score?.breakdown.floorNote ?? score?.breakdown.ceilingNote ?? null;

  return (
    <div className="panel panel--flush">
      <div className="issues-score-strip">
        <div className="issues-score-ring-wrap">
          <ScoreRing
            value={score?.sitecmdScore ?? null}
            total={100}
            labelMode="fraction"
            size={96}
          />
        </div>
        <div className="issues-score-copy">
          <span className="card__title">SiteCMD Score</span>
          <p className="text-body-muted">
            {score ? buildDetail(score, issueSummary) : "Loading score"} ·{" "}
            {formatCheckedAt(checkedAt, nowMs)}
          </p>
          {scoreNote ? (
            <p
              className={`text-meta ${score?.breakdown.capNote ? "text-severity-critical" : "text-muted-foreground"}`}>
              {scoreNote}
            </p>
          ) : null}
        </div>
      </div>
    </div>
  );
}
