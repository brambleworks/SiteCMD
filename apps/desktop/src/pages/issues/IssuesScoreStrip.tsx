import { PageGuideButton } from "@/components/layout/PageGuide";
import { ScoreRing } from "@/components/ui/score-ring";
import type { ProjectIssueSummary } from "@/lib/project-issue-summary";
import type { ScoreBreakdownDisplay } from "@/lib/score-breakdown";
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

/** The Rust-authored breakdown, shown as arithmetic a non-engineer can follow. */
function ScoreBreakdownDisclosure({ breakdown }: { breakdown: ScoreBreakdownDisplay }) {
  return (
    <details className="score-breakdown">
      <summary className="score-breakdown-summary">How this score is computed</summary>
      <ul className="score-breakdown-list">
        <li className="score-breakdown-row">
          <span>Starts at</span>
          <span className="score-breakdown-points">{breakdown.base}</span>
        </li>
        {breakdown.deductions.map((line) => (
          <li key={line.tier} className="score-breakdown-row">
            <span>{line.label} issues</span>
            <span className={`score-breakdown-points text-severity-${line.tier}`}>
              -{line.points}
            </span>
          </li>
        ))}
        {!breakdown.hasDeductions ? (
          <li className="score-breakdown-row">
            <span>No point deductions</span>
          </li>
        ) : null}
        {breakdown.floorNote ? (
          <li className="score-breakdown-note">{breakdown.floorNote}</li>
        ) : null}
        {breakdown.capNote ? (
          <li className="score-breakdown-note text-severity-critical">{breakdown.capNote}</li>
        ) : null}
        <li className="score-breakdown-row score-breakdown-total">
          <span>SiteCMD Score</span>
          <span className="score-breakdown-points">{breakdown.overall}</span>
        </li>
      </ul>
    </details>
  );
}

export function IssuesScoreStrip({ score, checkedAt, issueSummary = null }: IssuesScoreStripProps) {
  const nowMs = useCurrentTime();

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
          <div className="row-between">
            <span className="card__title">SiteCMD Score</span>
            <PageGuideButton page="score" />
          </div>
          <p className="text-body-muted">
            {score ? buildDetail(score, issueSummary) : "Loading score"} ·{" "}
            {formatCheckedAt(checkedAt, nowMs)}
          </p>
          {score?.breakdown.capNote ? (
            <p className="text-meta text-severity-critical">{score.breakdown.capNote}</p>
          ) : null}
          {score ? <ScoreBreakdownDisclosure breakdown={score.breakdown} /> : null}
        </div>
      </div>
    </div>
  );
}
