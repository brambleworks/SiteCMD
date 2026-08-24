import { Activity } from "lucide-react";
import { getScoreClass, type ScanExecutionSummary } from "@/lib/types";
import {
  createSeverityCounts,
  severityCountTotal,
  severityToneClass,
  type Severity,
} from "@/lib/severity";
import { cn } from "@/lib/utils";
import { SurfaceState } from "@/components/ui/surface-state";

interface ScanHistoryProps {
  executions: ScanExecutionSummary[];
  onOpenScanConfig?: () => void;
}

const SCAN_DATE_FORMATTER = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  year: "numeric",
  hour: "numeric",
  minute: "2-digit",
});

function preferredScanExecutions(executions: ScanExecutionSummary[]): ScanExecutionSummary[] {
  const scored = executions.filter((execution) => execution.score != null);
  const completedFullScans = scored.filter(
    (execution) => execution.requestedMode === "full" && execution.status === "complete",
  );
  return [...(completedFullScans.length > 0 ? completedFullScans : scored)].sort(
    (left, right) => right.startedAt - left.startedAt,
  );
}

function executionSeverityCounts(execution: ScanExecutionSummary) {
  return createSeverityCounts({
    critical: execution.criticalCount ?? 0,
    high: execution.highCount ?? 0,
    medium: execution.mediumCount ?? 0,
    low: execution.lowCount ?? 0,
  });
}

function SeverityCount({ count, severity }: { count: number; severity: Severity }) {
  return (
    <span className={cn(count > 0 ? severityToneClass(severity) : "text-muted-foreground")}>
      {count}
    </span>
  );
}

export function ScanHistory({ executions, onOpenScanConfig }: ScanHistoryProps) {
  const visibleExecutions = preferredScanExecutions(executions);

  if (visibleExecutions.length === 0) {
    return (
      <SurfaceState
        kind="empty"
        icon={<Activity className="empty-state-icon" />}
        title="No scans yet"
        description="Run your first scan to get a SiteCMD Score and a starting list of issues to fix."
        primaryAction={
          onOpenScanConfig ? { label: "Run First Scan", onClick: onOpenScanConfig } : undefined
        }
      />
    );
  }

  return (
    <div className="scan-history-panel">
      <div className="scan-history-head">
        <h3 className="row-title-md scan-history-title">Scans</h3>
        <span className="mono-subtle">{visibleExecutions.length}</span>
      </div>

      <div className="scan-history-table-shell">
        <table className="scan-history-table">
          <caption className="sr-only">SiteCMD scan history</caption>
          <thead className="scan-history-table-head">
            <tr>
              <th scope="col" className="scan-history-table-heading">
                Date
              </th>
              <th scope="col" className="scan-history-table-heading scan-history-table-heading-num">
                SiteCMD Score
              </th>
              <th scope="col" className="scan-history-table-heading scan-history-table-heading-num">
                Total Issues
              </th>
              <th scope="col" className="scan-history-table-heading scan-history-table-heading-num">
                Critical
              </th>
              <th scope="col" className="scan-history-table-heading scan-history-table-heading-num">
                High
              </th>
              <th scope="col" className="scan-history-table-heading scan-history-table-heading-num">
                Medium
              </th>
              <th scope="col" className="scan-history-table-heading scan-history-table-heading-num">
                Low
              </th>
            </tr>
          </thead>
          <tbody>
            {visibleExecutions.map((execution) => {
              const counts = executionSeverityCounts(execution);
              const score = Math.round(execution.score ?? 0);
              const startedAt = new Date(execution.startedAt);
              return (
                <tr
                  key={execution.id}
                  className="scan-history-table-row"
                  data-testid={`scan-history-row-${execution.id}`}>
                  <td className="scan-history-table-date">
                    <time dateTime={startedAt.toISOString()}>
                      {SCAN_DATE_FORMATTER.format(startedAt)}
                    </time>
                  </td>
                  <td className="scan-history-table-score">
                    <span className={getScoreClass(score)}>{score}</span>
                  </td>
                  <td className="scan-history-table-count">{severityCountTotal(counts)}</td>
                  <td className="scan-history-table-count">
                    <SeverityCount count={counts.critical} severity="critical" />
                  </td>
                  <td className="scan-history-table-count">
                    <SeverityCount count={counts.high} severity="high" />
                  </td>
                  <td className="scan-history-table-count">
                    <SeverityCount count={counts.medium} severity="medium" />
                  </td>
                  <td className="scan-history-table-count">
                    <SeverityCount count={counts.low} severity="low" />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
