import type { CodeIssue, CodeScanResult } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { ProgressBar } from "@/components/ui/progress-bar";
import { getScoreClass } from "@/lib/types";
import { getScoreCssVar } from "@/lib/tokens";
import {
  AlertCircle,
  CheckCircle,
  FileCode,
  FolderCode,
  Loader2,
  Minus,
  RefreshCw,
  ShieldAlert,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import { formatDelta, type CodeScanPresentation } from "@/components/scan/code-scan-result-model";
import type { CodeScanComparison } from "@/lib/code-scan-comparison";

export function CodeScanHeaderSection({
  currentResult,
  presentation,
  projectPath,
  topIssue,
  onFocusTopIssue,
  onOpenScanConfig,
}: {
  currentResult: CodeScanResult;
  presentation: CodeScanPresentation;
  projectPath?: string | null;
  topIssue: CodeIssue | null;
  onFocusTopIssue: () => void;
  onOpenScanConfig?: () => void;
}) {
  return (
    <section className="code-scan-header-grid">
      <div className="code-scan-header-main panel panel--muted panel--large">
        <div className="code-scan-header-row">
          <div className="code-scan-header-copy">
            <div className="code-scan-header-title-group">
              <label className="section-label-lg">Code Scan Target</label>
              <div className="code-scan-header-title-row">
                <h1 className="page-title-lg code-scan-header-title">{presentation.title}</h1>
                <FileCode className="icon-muted-sm code-scan-header-icon text-cat-code" />
              </div>
              <p className="subtitle-xs">
                Last checked on {new Date(currentResult.checkedAt).toLocaleString()}
                {currentResult.framework ? ` · ${currentResult.framework}` : ""}
              </p>
            </div>
            {projectPath ? <p className="subtitle-xs code-scan-path">{projectPath}</p> : null}
          </div>

          <div className="scan-metric-card">
            <label className="section-label-lg code-scan-score-label">
              {presentation.scoreLabel}
            </label>
            <span className={`code-scan-score ${getScoreClass(currentResult.overallScore)}`}>
              {currentResult.overallScore}
            </span>
            <ProgressBar
              percent={currentResult.overallScore}
              color={getScoreCssVar(currentResult.overallScore)}
              label={presentation.scoreLabel}
              trackClassName="code-scan-score-bar bg-background"
            />
          </div>
        </div>

        <div className="subtle-divider-top code-scan-header-actions row-between">
          <Button variant="outline" size="sm" onClick={onOpenScanConfig}>
            <RefreshCw className="icon-sm" />
            {presentation.rerunLabel}
          </Button>
          {topIssue ? (
            <Button variant="outline" size="sm" onClick={onFocusTopIssue}>
              <ShieldAlert className="icon-sm" />
              {presentation.focusLabel}
            </Button>
          ) : null}
        </div>
      </div>

      <div className="code-scan-header-side">
        <div className="panel panel--muted">
          <label className="section-label-lg code-scan-summary-label">Summary</label>
          <div className="code-scan-summary-list">
            <SummaryRow label="Total Issues" value={currentResult.issueCount} />
            {currentResult.criticalCount > 0 ? (
              <SummaryRow
                label="Critical"
                value={currentResult.criticalCount}
                className="text-severity-critical"
              />
            ) : null}
            {currentResult.highCount > 0 ? (
              <SummaryRow
                label="High"
                value={currentResult.highCount}
                className="text-severity-high"
              />
            ) : null}
            {currentResult.mediumCount > 0 ? (
              <SummaryRow
                label="Medium"
                value={currentResult.mediumCount}
                className="text-severity-medium"
              />
            ) : null}
            {currentResult.lowCount > 0 ? (
              <SummaryRow label="Low" value={currentResult.lowCount} />
            ) : null}
          </div>
        </div>

        {currentResult.durationMs > 0 ? (
          <div className="card card--muted code-scan-duration">
            <span className="section-label-lg">Duration</span>
            <div className="text-lg-bold code-scan-duration-value">
              {(currentResult.durationMs / 1000).toFixed(1)}s
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

/** Renders a compact delta card when a previous Code Scan exists. */
export function CodeScanComparisonSection({
  comparison,
  comparisonError,
  comparisonLoading,
  currentResult,
  onRetryComparison,
}: {
  comparison: CodeScanComparison | null;
  comparisonError: boolean;
  comparisonLoading: boolean;
  currentResult: CodeScanResult;
  onRetryComparison: () => void;
}) {
  if (comparisonError) {
    return (
      <div className="card card--muted row-between">
        <p className="body-muted">
          The previous Code Scan summary exists, but its issue snapshot could not load.
        </p>
        <Button size="sm" variant="outline" onClick={onRetryComparison}>
          Retry comparison
        </Button>
      </div>
    );
  }

  if (comparisonLoading) {
    return (
      <div className="code-scan-comparison-loading card-sunken body-muted">
        <Loader2 className="icon-md animate-spin text-cat-code" />
        Loading previous run comparison…
      </div>
    );
  }

  if (!comparison) return null;

  const { scoreDelta } = comparison;
  const deltaClass =
    scoreDelta > 0
      ? "text-score-excellent"
      : scoreDelta < 0
        ? "text-severity-critical"
        : "text-muted-foreground";

  return (
    <div className="card card--muted card--spacious">
      <label className="section-label-lg code-scan-changes-label">Changes Since Last Scan</label>
      <div className="code-scan-changes-row">
        <div className="code-scan-delta">
          {scoreDelta > 0 ? (
            <TrendingUp className="icon-lg text-score-excellent" />
          ) : scoreDelta < 0 ? (
            <TrendingDown className="icon-lg text-severity-critical" />
          ) : (
            <Minus className="icon-lg text-muted-foreground" />
          )}
          <div>
            <span className={`code-scan-delta-value ${deltaClass}`}>{formatDelta(scoreDelta)}</span>
            <span className="code-scan-delta-unit text-muted-foreground">pts</span>
            <p className="subtitle-xs">
              {currentResult.overallScore - scoreDelta} &rarr; {currentResult.overallScore}
            </p>
          </div>
        </div>
        <div className="code-scan-divider bg-muted" />
        <div className="code-scan-change-stats">
          {comparison.fixed.length > 0 ? (
            <div className="code-scan-change-stat">
              <CheckCircle className="icon-md text-score-excellent" />
              <span className="code-scan-change-count text-score-excellent">
                {comparison.fixed.length}
              </span>
              <span className="body-muted">fixed</span>
            </div>
          ) : null}
          {comparison.newIssues.length > 0 ? (
            <div className="code-scan-change-stat">
              <AlertCircle className="icon-md text-severity-critical" />
              <span className="code-scan-change-count text-severity-critical">
                {comparison.newIssues.length}
              </span>
              <span className="body-muted">new</span>
            </div>
          ) : null}
          {comparison.changed.length > 0 ? (
            <div className="code-scan-change-stat">
              <Minus className="icon-md text-muted-foreground" />
              <span className="code-scan-change-count text-foreground">
                {comparison.changed.length}
              </span>
              <span className="body-muted">changed</span>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

export function CodeScanEmptyState({ presentation }: { presentation: CodeScanPresentation }) {
  return (
    <div className="panel panel--muted panel--empty code-scan-empty">
      <FolderCode className="code-scan-empty-icon text-score-excellent" />
      <p className="text-sm-bold">{presentation.emptyTitle}</p>
      <p className="body-muted code-scan-empty-copy">{presentation.emptyCopy}</p>
    </div>
  );
}

function SummaryRow({
  label,
  value,
  className = "",
}: {
  label: string;
  value: number;
  className?: string;
}) {
  return (
    <div className="row-between">
      <span className="code-scan-summary-row-label text-muted-foreground">{label}</span>
      <span className={`code-scan-summary-row-value ${className}`}>{value}</span>
    </div>
  );
}
