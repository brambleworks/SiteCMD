import { ArrowRight, CheckCircle2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { ScoreRing } from "@/components/ui/score-ring";
import { type ScanSummaryModel } from "@/components/scan/scan-summary-model";

interface ScanSummaryOverlayProps {
  summary: ScanSummaryModel;
  onClose: () => void;
  onReviewIssues: () => void;
}

function pluralize(count: number, singular: string, plural = `${singular}s`) {
  return `${count} ${count === 1 ? singular : plural}`;
}

function displayChangeCount(value: number | null) {
  return value == null ? "-" : String(value);
}

export function ScanSummaryOverlay({ summary, onClose, onReviewIssues }: ScanSummaryOverlayProps) {
  const hasIssues = summary.totalIssues > 0;

  return (
    <Dialog labelledBy="scan-summary-title" onClose={onClose} className="scan-summary-panel">
      <div className="scan-summary-header">
        <div className="row-loose flex-fill">
          <span className="scan-summary-icon">
            <CheckCircle2 className="icon-lg" aria-hidden="true" />
          </span>
          <div className="flex-fill">
            <p className="section-label">Scan Summary</p>
            <h2 id="scan-summary-title" className="scan-summary-title">
              {summary.title}
            </h2>
            <p className="scan-summary-scope">{summary.scopeLabel}</p>
          </div>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="icon-btn"
          aria-label="Close scan summary"
          onClick={onClose}>
          <X className="icon-md" aria-hidden="true" />
        </Button>
      </div>

      <div className="scan-summary-body">
        <div className="scan-summary-hero">
          <div className="scan-summary-hero-ring">
            <ScoreRing value={summary.siteCmdScore} labelMode="fraction" size={108} />
            <p className="section-label">SiteCMD Score</p>
          </div>
          <div className="flex-fill">
            <p className="scan-summary-lede">
              {hasIssues
                ? `${pluralize(summary.totalIssues, "open issue")} after this scan`
                : "No open issues after this scan"}
            </p>
            <div className="scan-summary-stat-grid">
              <SummaryStat label="New" value={displayChangeCount(summary.estimatedNewIssues)} />
              <SummaryStat label="Resolved" value={displayChangeCount(summary.resolvedIssues)} />
              <SummaryStat
                label="Regressions"
                value={displayChangeCount(summary.regressionCount)}
              />
            </div>
          </div>
        </div>

        {summary.incompleteDetail ? (
          <p className="scan-summary-section-note">{summary.incompleteDetail}</p>
        ) : null}
        {summary.note ? <p className="scan-summary-section-note">{summary.note}</p> : null}

        <div className="scan-summary-severity">
          <p className="section-label">Issues by severity</p>
          <div className="scan-summary-severity-row" aria-label="Issue severity totals">
            <SeverityCount
              label="Critical"
              value={summary.severityCounts.critical}
              tone="critical"
            />
            <SeverityCount label="High" value={summary.severityCounts.high} tone="high" />
            <SeverityCount label="Medium" value={summary.severityCounts.medium} tone="medium" />
            <SeverityCount label="Low" value={summary.severityCounts.low} tone="low" />
          </div>
        </div>
      </div>

      <div className="scan-summary-footer">
        <Button type="button" variant="ghost" onClick={onClose}>
          Close
        </Button>
        <Button type="button" onClick={onReviewIssues} disabled={!hasIssues}>
          Review Issues
          <ArrowRight className="icon-md" aria-hidden="true" />
        </Button>
      </div>
    </Dialog>
  );
}

function SummaryStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="scan-summary-stat">
      <span className="scan-summary-stat-value">{value}</span>
      <span className="scan-summary-stat-label">{label}</span>
    </div>
  );
}

function SeverityCount({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone: "critical" | "high" | "medium" | "low";
}) {
  return (
    <div className={`scan-summary-severity-item scan-summary-severity-${tone}`}>
      <span className="scan-summary-severity-value">{value}</span>
      <span className="scan-summary-severity-label">{label}</span>
    </div>
  );
}
