import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Loader2, Send } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { getIssueLinkForCheck } from "@/lib/commands";
import {
  enabledTrackerProviders,
  sendIssueToTracker,
  TRACKER_LABELS,
  type TrackerProvider,
} from "@/lib/issue-links";
import { openUrl } from "@/lib/open-url";
import type { CheckResult, IssueLink } from "@/lib/types";
import { useIntegrationsQuery } from "@/hooks/useIntegrationsQuery";
import { queryKeys } from "@/lib/query/query-keys";

interface SendToTrackerActionProps {
  projectId: number;
  issue: CheckResult;
  /** Scan the issue came from; the action hides until a scan id exists. */
  scanId: number | null;
  /** Estimated score points this issue costs; rendered in the ticket body. */
  estimatedImpact: number;
  /** Fired after a ticket is created so callers can refresh link chips. */
  onLinkCreated?: (link: IssueLink) => void;
}

/** Tracker action that becomes a ticket link after one issue is mirrored. */
export function SendToTrackerAction({
  projectId,
  issue,
  scanId,
  estimatedImpact,
  onLinkCreated,
}: SendToTrackerActionProps) {
  const { success, error: toastError } = useToast();
  const queryClient = useQueryClient();
  const issueLinkQueryKey = queryKeys.issueLinks.forCheck(projectId, issue.checkId);
  const linkQuery = useQuery<IssueLink | null>({
    queryKey: issueLinkQueryKey,
    queryFn: () => getIssueLinkForCheck({ projectId, checkId: issue.checkId }),
  });
  const integrationsQuery = useIntegrationsQuery(projectId);
  const providers = useMemo(
    () => (integrationsQuery.loading ? null : enabledTrackerProviders(integrationsQuery.configs)),
    [integrationsQuery.configs, integrationsQuery.loading],
  );
  const link = linkQuery.data ?? null;
  const loadError = linkQuery.isError || integrationsQuery.error != null;

  const [sending, setSending] = useState<TrackerProvider | null>(null);

  const disposedRef = useRef(false);
  useEffect(() => {
    disposedRef.current = false;
    return () => {
      disposedRef.current = true;
    };
  }, []);

  const handleSend = async (provider: TrackerProvider) => {
    if (sending !== null || scanId === null) return;
    setSending(provider);
    try {
      const created = await sendIssueToTracker({
        projectId,
        scanId,
        provider,
        issue,
        estimatedImpact,
      });
      queryClient.setQueryData(issueLinkQueryKey, created);
      success(
        "Ticket created",
        `${TRACKER_LABELS[provider]} ${created.externalId} now tracks this issue.`,
      );
      onLinkCreated?.(created);
    } catch (err) {
      toastError("Could not create the ticket", String(err));
    } finally {
      if (!disposedRef.current) setSending(null);
    }
  };

  if (loadError) {
    return (
      <div className="dossier-rail-list">
        <p className="body-muted">Ticket status could not load.</p>
        <Button
          size="sm"
          variant="outline"
          onClick={() => {
            void linkQuery.refetch();
            void integrationsQuery.reload();
          }}>
          Retry
        </Button>
      </div>
    );
  }

  // Loading, or trackers not relevant here: render nothing rather than a
  // placeholder so the Actions rail stays quiet.
  if (providers === null) return null;

  if (link) {
    const label = TRACKER_LABELS[link.provider as TrackerProvider] ?? link.provider;
    return (
      <Button
        variant="outline"
        className="btn--block"
        onClick={() => void openUrl(link.externalUrl)}
        aria-label={`Open ${label} ticket ${link.externalId}`}>
        <ExternalLink className="icon-sm" />
        <span>
          {label} {link.externalId}
        </span>
      </Button>
    );
  }

  if (providers.length === 0) {
    // No tracker connected: render nothing. Users connect a tracker from the
    // Integrations page.
    return null;
  }

  if (scanId === null) return null;

  return (
    <>
      {providers.map((provider) => (
        <Button
          key={provider}
          variant="outline"
          className="btn--block"
          onClick={() => void handleSend(provider)}
          disabled={sending !== null}
          aria-label={`Send to ${TRACKER_LABELS[provider]}`}>
          {sending === provider ? <Loader2 className="spinner-sm" /> : <Send className="icon-sm" />}
          <span>Send to {TRACKER_LABELS[provider]}</span>
        </Button>
      ))}
    </>
  );
}
