import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { ConnectedReportLink } from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { copyToClipboard } from "@/lib/clipboard";
import { createConnectedReport, listConnectedReports, revokeConnectedReport } from "@/lib/commands";
import { formatRelativeTime } from "@/lib/format";
import { queryKeys } from "@/lib/query/query-keys";
import { useCurrentTime } from "@/lib/useCurrentTime";

interface ConnectedReportsSectionProps {
  projectId: number;
  environmentScopeKey: string;
}

const EXPIRY_CHOICES = [
  { days: 7, label: "7 days" },
  { days: 30, label: "30 days" },
  { days: 90, label: "90 days" },
];

function expiryDate(isoDate: string): string {
  const parsed = Date.parse(isoDate);
  return Number.isFinite(parsed) ? new Date(parsed).toLocaleDateString() : "unknown";
}

/** Manage report links whose full URL appears only at creation. */
export function ConnectedReportsSection({
  projectId,
  environmentScopeKey,
}: ConnectedReportsSectionProps) {
  const nowMs = useCurrentTime();
  const toast = useToast();
  const queryClient = useQueryClient();
  const queryKey = queryKeys.settings.connectedReports(projectId, environmentScopeKey);
  const scope = { environmentScopeKey, projectId };
  const registryQuery = useQuery({
    queryKey,
    queryFn: () => listConnectedReports(scope),
  });
  const [includeRoutes, setIncludeRoutes] = useState(false);
  const [ttlDays, setTtlDays] = useState(30);
  const [creating, setCreating] = useState(false);
  const [createdLink, setCreatedLink] = useState<ConnectedReportLink | null>(null);
  const [revoking, setRevoking] = useState<string | null>(null);

  const handleCreate = async () => {
    setCreating(true);
    try {
      const created = await createConnectedReport({ ...scope, includeRoutes, ttlDays });
      setCreatedLink(created);
      await queryClient.invalidateQueries({ queryKey });
      toast.success("Report link created", "Copy it now. The registry never shows it again.");
    } catch (error) {
      toast.error("Could not create the report link", String(error));
    } finally {
      setCreating(false);
    }
  };

  const handleRevoke = async (reportId: string) => {
    setRevoking(reportId);
    try {
      await revokeConnectedReport({ ...scope, reportId });
      if (createdLink?.reportId === reportId) setCreatedLink(null);
      await queryClient.invalidateQueries({ queryKey });
      toast.success("Report link revoked", "The link stops opening immediately.");
    } catch (error) {
      toast.error("Could not revoke the report link", String(error));
    } finally {
      setRevoking(null);
    }
  };

  const reports = registryQuery.data ?? [];

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Shareable Reports</h2>
      </div>
      <p className="body-muted">
        A report link opens a snapshot of this site's health without an account: score, severity and
        category counts, verification wins, and trends. It is frozen when you create it, expires on
        its own, and never contains evidence or code. Route-level detail is the one extra you opt
        into.
      </p>
      <div className="stack-base connected-form">
        <label className="rep-attr-label">
          <input
            type="checkbox"
            checked={includeRoutes}
            onChange={(event) => setIncludeRoutes(event.target.checked)}
            className="rep-checkbox"
          />
          <span className="text-13-muted">Include route-level detail</span>
        </label>
        <label className="form-label" htmlFor="connected-report-expiry">
          Link expires after
        </label>
        <select
          id="connected-report-expiry"
          value={ttlDays}
          onChange={(event) => setTtlDays(Number(event.target.value))}
          className="field-control field-control--muted field-control--select">
          {EXPIRY_CHOICES.map((choice) => (
            <option key={choice.days} value={choice.days}>
              {choice.label}
            </option>
          ))}
        </select>
        <Button onClick={() => void handleCreate()} disabled={creating}>
          {creating ? "Creating..." : "Create Report Link"}
        </Button>
      </div>
      {createdLink ? (
        <div className="connected-payload-wrap">
          <div className="row-between">
            <p className="text-13-medium">Copy this link now</p>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void copyToClipboard(createdLink.link)}>
              Copy Link
            </Button>
          </div>
          <pre className="connected-payload">{createdLink.link}</pre>
          <p className="text-body-muted">
            The registry below tracks this link but never shows it again. It expires on{" "}
            {expiryDate(createdLink.expiresAt)}.
          </p>
        </div>
      ) : null}
      {registryQuery.isError ? (
        <p className="agent-handoff-error" role="alert">
          The report registry could not load.
        </p>
      ) : null}
      {reports.length > 0 ? (
        <div className="webhook-list">
          {reports.map((report) => (
            <div key={report.reportId} className="settings-webhook-row">
              <span
                className={report.revoked ? "status-dot-info status-dot-dim" : "status-dot-success"}
              />
              <span className="text-13-muted flex-fill">
                Created {formatRelativeTime(report.createdAt, nowMs)}
                {report.includeRoutes ? ", with route detail" : ""}
                {report.revoked ? ", revoked" : `, expires ${expiryDate(report.expiresAt)}`}
              </span>
              <span className="text-13-muted">
                {report.viewCount} view{report.viewCount === 1 ? "" : "s"}
              </span>
              {!report.revoked ? (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleRevoke(report.reportId)}
                  disabled={revoking === report.reportId}>
                  {revoking === report.reportId ? "Revoking..." : "Revoke"}
                </Button>
              ) : null}
            </div>
          ))}
        </div>
      ) : registryQuery.isSuccess ? (
        <p className="body-muted">No report links exist yet. Create the first one above.</p>
      ) : null}
    </section>
  );
}
