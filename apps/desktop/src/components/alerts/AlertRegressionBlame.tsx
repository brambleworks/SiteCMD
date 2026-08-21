import { useState } from "react";
import { Button } from "@/components/ui/button";
import type { DeployRegressionDetail } from "./alert-detail-model";

interface Props {
  detail: DeployRegressionDetail;
  onOpenIssues?: () => void;
}

export function AlertRegressionBlame({ detail, onOpenIssues }: Props) {
  const [showAllCommits, setShowAllCommits] = useState(false);
  const shortFrom = detail.commitFrom.slice(0, 7);
  const shortTo = detail.commitTo.slice(0, 7);
  const visibleCommits = showAllCommits ? detail.commits : detail.commits.slice(0, 3);
  const hiddenCount = detail.commits.length - visibleCommits.length;
  const hasExpander = detail.commits.length > 3;
  // Flag truncated blame only after all stored commits are visible.
  const showCapNote =
    detail.commitCount > detail.commits.length && (showAllCommits || !hasExpander);

  return (
    <div className="alert-blame">
      <p className="alert-blame-headline">
        {detail.scoreDrop > 0
          ? `Score went from ${detail.previousScore} to ${detail.currentScore} (down ${detail.scoreDrop} ${detail.scoreDrop === 1 ? "point" : "points"}) after ${detail.commitCount} ${detail.commitCount === 1 ? "commit" : "commits"} landed.`
          : `${detail.newIssues.length} new ${detail.newIssues.length === 1 ? "issue" : "issues"} appeared after ${detail.commitCount} ${detail.commitCount === 1 ? "commit" : "commits"} landed, even though the score held.`}
      </p>

      <div className="alert-blame-range">
        <span className="alert-blame-range-label">Blame window</span>
        <code className="alert-blame-range-shas">
          {shortFrom}..{shortTo}
        </code>
        <span className="alert-blame-range-count">
          {detail.commitCount} {detail.commitCount === 1 ? "commit" : "commits"}
        </span>
      </div>

      <ul className="alert-blame-commits">
        {visibleCommits.map((commit) => (
          <li key={commit.hash} className="alert-blame-commit">
            <code className="alert-blame-commit-sha">{commit.shortHash}</code>
            <span className="alert-blame-commit-message">{commit.message}</span>
            <span className="alert-blame-commit-author">{commit.author}</span>
          </li>
        ))}
      </ul>
      {hasExpander ? (
        <Button
          unstyled
          type="button"
          className="alert-blame-more"
          onClick={() => setShowAllCommits((expanded) => !expanded)}>
          {showAllCommits
            ? "Show fewer commits"
            : `Show ${hiddenCount} more ${hiddenCount === 1 ? "commit" : "commits"}`}
        </Button>
      ) : null}
      {showCapNote ? (
        <p className="alert-blame-capped">
          Newest {detail.commits.length} of {detail.commitCount} commits shown.
        </p>
      ) : null}

      <div className="alert-blame-issues">
        <p className="alert-blame-issues-label">
          {detail.newIssues.length} new {detail.newIssues.length === 1 ? "issue" : "issues"}{" "}
          introduced
        </p>
        <ul>
          {detail.newIssues.map((issue) => (
            <li key={issue.checkId} className="alert-blame-issue">
              {onOpenIssues ? (
                <Button
                  unstyled
                  type="button"
                  className="alert-blame-issue-link"
                  onClick={onOpenIssues}>
                  {issue.title}
                </Button>
              ) : (
                <span>{issue.title}</span>
              )}
            </li>
          ))}
        </ul>
        {detail.fixedCount > 0 ? (
          <p className="alert-blame-fixed">
            Also fixed {detail.fixedCount} {detail.fixedCount === 1 ? "issue" : "issues"}.
          </p>
        ) : null}
        {detail.detectorChangedCount > 0 ? (
          <p className="alert-blame-withheld">
            {detail.detectorChangedCount} other{" "}
            {detail.detectorChangedCount === 1 ? "finding comes" : "findings come"} from{" "}
            {detail.detectorChangedCount === 1 ? "a check" : "checks"} that changed in
            {detail.engineRelease ? ` SiteCMD ${detail.engineRelease}` : " this release"}, so{" "}
            {detail.detectorChangedCount === 1 ? "it is" : "they are"} not attributed to these
            commits.
          </p>
        ) : null}
      </div>
    </div>
  );
}
