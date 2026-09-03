import type { RefObject } from "react";
import { Download, FileText, Loader2, RotateCcw, Trash2 } from "lucide-react";
import { HeaderActions } from "@/app/ShellHeader";
import { Button } from "@/components/ui/button";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import {
  formatSavedReportDate,
  getHistoryCoverageBadges,
  parseHistorySections,
  parseHistorySummary,
  reportFormatLabel,
  type ReportHistoryEntry,
} from "@/components/reports/reports-page-model";

export function ReportsBuilderLoadingState() {
  return (
    <LoadingRegion label="Reports loading state" className="page-content stack-hero">
      <div className="row-end">
        <Skeleton className="rep-sk-header-btn" />
      </div>

      <div className="rep-2col-grid">
        {[0, 1].map((index) => (
          <div key={index} className="card card--muted card--spacious">
            <Skeleton className="rep-sk-label" />
            {index === 0 ? (
              <>
                <Skeleton className="rep-sk-input" />
                <Skeleton className="rep-sk-line" />
              </>
            ) : (
              <div className="row">
                {[0, 1, 2].map((button) => (
                  <Skeleton key={button} className="rep-sk-btn-fill" />
                ))}
              </div>
            )}
          </div>
        ))}
      </div>

      <div className="card card--muted card--spacious">
        <div className="rep-sk-head">
          <div className="stack-snug">
            <Skeleton className="rep-sk-line-wide" />
            <Skeleton className="rep-sk-line-full" />
            <Skeleton className="rep-sk-line-83" />
          </div>
          <Skeleton className="rep-sk-badge" />
        </div>
        <div className="rep-sk-badges">
          {[0, 1, 2, 3, 4].map((index) => (
            <Skeleton key={index} className="rep-sk-badge-2" />
          ))}
        </div>
      </div>

      <div className="card card--muted card--spacious">
        <div className="rep-sk-head">
          <div className="stack-snug">
            <Skeleton className="rep-sk-line-36" />
            <Skeleton className="rep-sk-line-full" />
            <Skeleton className="rep-sk-line-80" />
          </div>
          <Skeleton className="rep-sk-btn-sm" />
        </div>
        <div className="rep-sk-metric-grid">
          {[0, 1, 2].map((index) => (
            <div key={index} className="card">
              <Skeleton className="rep-sk-metric-label" />
              <Skeleton className="rep-sk-metric-value" />
              <Skeleton className="rep-sk-metric-line" />
              <Skeleton className="rep-sk-metric-line-80" />
            </div>
          ))}
        </div>
      </div>

      <div className="panel panel--flush panel--muted">
        <div className="row-between rep-panel-head">
          <div className="row-loose">
            <Skeleton className="rep-sk-dot" />
            <Skeleton className="rep-sk-panel-title" />
          </div>
          <Skeleton className="rep-sk-dot" />
        </div>
        <div className="rep-panel-body">
          <div className="rep-panel-grid">
            {[0, 1].map((index) => (
              <div key={index}>
                <Skeleton className="rep-sk-field-label" />
                <Skeleton className="rep-sk-field" />
              </div>
            ))}
          </div>
          <Skeleton className="rep-sk-field" />
          <Skeleton className="rep-sk-line-64" />
        </div>
      </div>

      <div className="panel panel--flush panel--muted">
        <div className="row-between rep-panel-head">
          <div className="row-loose">
            <Skeleton className="rep-sk-dot" />
            <Skeleton className="rep-sk-panel-title" />
          </div>
          <Skeleton className="rep-sk-dot" />
        </div>
        <div className="builder-action-grid">
          {[0, 1, 2, 3, 4, 5].map((index) => (
            <Skeleton key={index} className="rep-sk-action" />
          ))}
        </div>
      </div>
    </LoadingRegion>
  );
}

export function ReportsPreview({
  generating,
  iframeRef,
  onClose,
  onExportPDF,
  onSaveHtml,
  previewHtml,
}: {
  generating: boolean;
  iframeRef: RefObject<HTMLIFrameElement | null>;
  onClose: () => void;
  onExportPDF: () => void;
  onSaveHtml: () => void;
  previewHtml: string;
}) {
  return (
    <div className="page-content stack-card">
      <HeaderActions>
        <Button onClick={onExportPDF} disabled={generating} className="btn--gap-snug">
          {generating ? (
            <Loader2 className="icon-md animate-spin" />
          ) : (
            <Download className="icon-md" />
          )}{" "}
          Export PDF
        </Button>
        <Button variant="outline" size="sm" onClick={onSaveHtml} className="btn--gap-tight">
          <Download className="icon-sm" /> Save HTML
        </Button>
        <Button variant="ghost" size="sm" onClick={onClose}>
          Close
        </Button>
      </HeaderActions>
      <div className="report-preview-shell">
        <iframe
          ref={iframeRef}
          srcDoc={previewHtml}
          sandbox=""
          className="report-preview-frame report-iframe"
          title="Report Preview"
        />
      </div>
    </div>
  );
}

export function ReportsHistorySection({
  error = null,
  history,
  loading = false,
  onDelete,
  onRegenerate,
  onRetry,
}: {
  error?: string | null;
  history: ReportHistoryEntry[];
  loading?: boolean;
  onDelete: (entry: ReportHistoryEntry) => void;
  onRegenerate: (entry: ReportHistoryEntry) => void;
  onRetry?: () => void;
}) {
  if (loading && history.length === 0) {
    return (
      <LoadingRegion label="Report history loading state">
        <div className="rep-history-head">
          <span className="eyebrow--alt text-muted-foreground no-shrink">Report History</span>
          <div className="rep-history-rule" />
        </div>
        <div className="panel panel--flush panel--muted">
          {[0, 1].map((index) => (
            <div key={index} className="list-row-hover">
              <Skeleton className="rep-sk-hist-icon" />
              <div className="flex-fill stack-snug">
                <Skeleton className="rep-sk-hist-title" />
                <Skeleton className="rep-sk-hist-sub" />
              </div>
            </div>
          ))}
        </div>
      </LoadingRegion>
    );
  }

  if (error && history.length === 0) {
    return (
      <div>
        <div className="rep-history-head">
          <span className="eyebrow--alt text-muted-foreground no-shrink">Report History</span>
          <div className="rep-history-rule" />
        </div>
        <div className="panel panel--flush panel--muted row-between rep-history-error" role="alert">
          <p className="text-body-muted text-severity-critical">{error}</p>
          {onRetry ? (
            <Button variant="outline" size="sm" onClick={onRetry}>
              Retry
            </Button>
          ) : null}
        </div>
      </div>
    );
  }

  if (history.length === 0) return null;

  return (
    <div>
      <div className="rep-history-head">
        <span className="eyebrow--alt text-muted-foreground no-shrink">Report History</span>
        <div className="rep-history-rule" />
      </div>
      <div className="panel panel--flush panel--muted">
        {history.map((entry, i) => {
          const savedSections = parseHistorySections(entry.sectionsJson);
          const summary = parseHistorySummary(entry.reportSummaryJson);
          const siteScanDate = formatSavedReportDate(summary?.latest_scan_date ?? null);
          const codeScanDate = formatSavedReportDate(summary?.code_checked_at ?? null);
          return (
            <div
              key={entry.id}
              className={`group list-row-hover ${i > 0 ? "subtle-divider-top" : ""}`}>
              <div className="icon-badge icon-badge--md icon-badge--primary-strong">
                <FileText className="icon-18 text-primary" />
              </div>
              <div className="flex-fill">
                <p className="row-title-md text-truncate">{entry.reportTitle}</p>
                <p className="subtitle-xs">
                  {new Date(entry.generatedAt + "Z").toLocaleDateString(undefined, {
                    month: "short",
                    day: "numeric",
                    year: "numeric",
                    hour: "numeric",
                    minute: "2-digit",
                  })}
                  {" · "}Last {entry.periodDays} days
                  {" · "}
                  {reportFormatLabel(entry.outputFormat)}
                </p>
                {summary && (
                  <p className="text-body-muted rep-history-meta">
                    SiteCMD Score {summary.site_score}/100 · {summary.site_critical} critical ·{" "}
                    {summary.site_high} high
                    {savedSections.code_scan && summary.has_code_scan
                      ? ` · code ${summary.code_critical ?? 0} critical · ${summary.code_high ?? 0} high`
                      : ""}
                    {savedSections.code_scan && summary.code_top_domain
                      ? ` · ${summary.code_top_domain}`
                      : ""}
                    {savedSections.code_scan && summary.code_domain_trend
                      ? ` · ${summary.code_domain_trend}`
                      : ""}
                  </p>
                )}
                {(siteScanDate || (savedSections.code_scan && codeScanDate)) && (
                  <p className="text-meta rep-history-meta">
                    {siteScanDate ? `Web scan ${siteScanDate}` : ""}
                    {siteScanDate && savedSections.code_scan && codeScanDate ? " · " : ""}
                    {savedSections.code_scan && codeScanDate ? `Code scan ${codeScanDate}` : ""}
                  </p>
                )}
                <div className="rep-history-badges">
                  {getHistoryCoverageBadges(savedSections, summary).map((badge) => (
                    <span
                      key={`${entry.id}-${badge.label}`}
                      className={`text-micro rep-history-badge ${
                        badge.tone === "primary"
                          ? "text-primary"
                          : badge.tone === "warning"
                            ? "text-severity-medium"
                            : "text-muted-foreground"
                      }`}>
                      {badge.label}
                    </span>
                  ))}
                </div>
              </div>
              <Button
                unstyled
                onClick={() => onRegenerate(entry)}
                className="icon-btn-reveal"
                title="Regenerate">
                <RotateCcw className="icon-muted-sm" />
              </Button>
              <Button
                unstyled
                onClick={() => onDelete(entry)}
                className="icon-btn-reveal"
                title="Delete">
                <Trash2 className="icon-muted-sm rep-delete-icon" />
              </Button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
