import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  BellOff,
  BellRing,
  GitBranch,
  RefreshCw,
  SearchCheck,
  ShieldAlert,
  TrendingDown,
  Wifi,
} from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { AlertList } from "@/components/alerts/AlertList";
import { AlertDossier } from "@/components/alerts/AlertDossier";
import {
  ALERT_SOURCE_DEFINITIONS,
  NATIVE_ALERT_DEFINITIONS,
  type AlertSourceDefinition,
  type NativeAlertDefinition,
} from "@/components/alerts/alert-display";
import { ConnectedAlertTimeline } from "@/components/alerts/ConnectedAlertTimeline";
import type { ConnectedAlertElsewhere } from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { InlineSkeleton, LoadingRegion } from "@/components/ui/skeleton";
import { listConnectedDestinations, refreshEvents } from "@/lib/commands";
import { publishEventsRecorded } from "@/lib/event-writes";
import { publishAlertsChanged, useAlerts } from "@/hooks/useAlerts";
import { useConnectedAlerts } from "@/hooks/useConnectedAlerts";
import { queryKeys } from "@/lib/query/query-keys";
import type { AlertFilter, AlertRow } from "@/lib/types";
import type { NavTarget } from "@/components/layout/nav-page";
import { useIntegrationsQuery } from "@/hooks/useIntegrationsQuery";
import { CONNECTED_ALERT_UNAVAILABLE, CONNECTED_LINK_UNKNOWN } from "@/lib/deep-links";

interface AlertsPageProps {
  projectId: number;
  environmentScopeKey: string;
  onNavigate?: (page: NavTarget) => void;
  deepLinkTarget?: {
    alertId?: string | null;
    reason?: string | null;
    arrival?: number;
  } | null;
}

const SOURCE_ICONS: Record<string, typeof Wifi> = {
  uptimerobot: Wifi,
  cloudflare: ShieldAlert,
  plausible: TrendingDown,
  ga4: TrendingDown,
  gsc: SearchCheck,
  github: GitBranch,
};

const NATIVE_ALERT_ICONS: Record<string, typeof Wifi> = {
  "web-regressions": BellRing,
  "code-regressions": TrendingDown,
  "scan-failures": RefreshCw,
  "dependency-updates": ShieldAlert,
};

export function AlertsPage({
  projectId,
  environmentScopeKey,
  onNavigate,
  deepLinkTarget,
}: AlertsPageProps) {
  const [filter, setFilter] = useState<AlertFilter>("all");
  const {
    alerts,
    unreadCount,
    loading,
    error,
    refresh,
    dismiss,
    markViewed,
    markUnread,
    markAllRead,
  } = useAlerts(projectId, filter);
  const {
    configs,
    error: integrationsError,
    loading: integrationsLoading,
  } = useIntegrationsQuery(projectId);
  const connected = useConnectedAlerts(projectId, environmentScopeKey);
  // Non-admin timelines omit address data and share the Settings query key.
  const destinationsQuery = useQuery({
    queryKey: queryKeys.settings.connectedDestinations(),
    queryFn: () => listConnectedDestinations(),
    enabled: connected.feed.availability === "ready",
  });
  const [selectedAlert, setSelectedAlert] = useState<AlertRow | null>(null);
  const [selectedConnectedAlertId, setSelectedConnectedAlertId] = useState<string | null>(null);
  const [selectedAnchorIndex, setSelectedAnchorIndex] = useState<number | null>(null);
  const connectedTypes = useMemo(
    () =>
      new Set(configs.filter((config) => config.enabled).map((config) => config.integrationType)),
    [configs],
  );
  const [checkingSources, setCheckingSources] = useState(false);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [sourcesExpanded, setSourcesExpanded] = useState(false);
  const deepLinkAlertId = deepLinkTarget?.alertId ?? null;
  const deepLinkReason = deepLinkTarget?.reason ?? null;
  const deepLinkArrival = deepLinkTarget?.arrival ?? 0;
  const [dismissedDeepLinkKey, setDismissedDeepLinkKey] = useState<string | null>(null);
  const appliedDeepLinkRef = useRef<string | null>(null);
  // Resolve alert links locally, then within the current site, then account-wide.
  const deepLinkMatch = useMemo(
    () =>
      deepLinkAlertId ? (alerts.find((alert) => alert.alertId === deepLinkAlertId) ?? null) : null,
    [alerts, deepLinkAlertId],
  );
  const deepLinkConnectedMatch = useMemo(
    () =>
      deepLinkAlertId
        ? (connected.feed.alerts.find((alert) => alert.alertId === deepLinkAlertId) ?? null)
        : null,
    [connected.feed.alerts, deepLinkAlertId],
  );
  const deepLinkElsewhere = useMemo(
    () =>
      deepLinkAlertId
        ? (connected.feed.elsewhere.find((entry) => entry.alertId === deepLinkAlertId) ?? null)
        : null,
    [connected.feed.elsewhere, deepLinkAlertId],
  );

  const displayAlerts = useMemo(() => {
    if (!selectedAlert || selectedAnchorIndex == null) return alerts;
    const rowsWithoutSelected = alerts.filter((alert) => alert.id !== selectedAlert.id);
    const targetIndex = Math.min(selectedAnchorIndex, rowsWithoutSelected.length);
    return [
      ...rowsWithoutSelected.slice(0, targetIndex),
      selectedAlert,
      ...rowsWithoutSelected.slice(targetIndex),
    ];
  }, [alerts, selectedAlert, selectedAnchorIndex]);

  const handleFilterChange = useCallback((nextFilter: AlertFilter) => {
    setFilter(nextFilter);
    setSelectedAlert(null);
    setSelectedAnchorIndex(null);
  }, []);

  const handleSelectAlert = useCallback(
    (alert: AlertRow) => {
      const isUnread = alert.viewedAt === null && alert.dismissedAt === null;
      const openedAt = Date.now();
      const anchorIndex = alerts.findIndex((row) => row.id === alert.id);
      setSelectedAnchorIndex(anchorIndex >= 0 ? anchorIndex : null);
      setSelectedAlert(isUnread ? { ...alert, viewedAt: openedAt } : alert);
      if (!isUnread) return;

      void Promise.resolve(markViewed(alert.id)).catch((err) => {
        setSourceError(String(err));
        setSelectedAlert((current) => (current?.id === alert.id ? alert : current));
      });
    },
    [markViewed, alerts],
  );

  // Handle each deep link once, even if its rows refetch after dismissal.
  const deepLinkKey = `${deepLinkArrival}|${deepLinkAlertId ?? ""}|${deepLinkReason ?? ""}`;
  useEffect(() => {
    if (appliedDeepLinkRef.current === deepLinkKey) return;
    if (deepLinkMatch) {
      appliedDeepLinkRef.current = deepLinkKey;
      // eslint-disable-next-line react-hooks/set-state-in-effect -- apply an imperative deep-link selection once
      handleSelectAlert(deepLinkMatch);
      return;
    }
    if (deepLinkConnectedMatch) {
      appliedDeepLinkRef.current = deepLinkKey;
      setSelectedConnectedAlertId(deepLinkConnectedMatch.alertId);
    }
  }, [deepLinkConnectedMatch, deepLinkKey, deepLinkMatch, handleSelectAlert]);

  // An alert id is unknown only after both timelines finish loading.
  const resolvingDeepLink = loading || connected.loading;
  const showDeepLinkNotice =
    dismissedDeepLinkKey !== deepLinkKey &&
    (deepLinkReason != null ||
      (deepLinkAlertId != null && !resolvingDeepLink && !deepLinkMatch && !deepLinkConnectedMatch));

  const handleMarkViewedSelected = useCallback(
    (alert: AlertRow) => {
      const viewedAt = Date.now();
      setSelectedAlert((current) =>
        current?.id === alert.id ? { ...current, viewedAt } : current,
      );
      void Promise.resolve(markViewed(alert.id)).catch((err) => {
        setSourceError(String(err));
        setSelectedAlert((current) =>
          current?.id === alert.id ? { ...current, viewedAt: alert.viewedAt } : current,
        );
      });
    },
    [markViewed],
  );

  const handleMarkUnreadSelected = useCallback(
    (alert: AlertRow) => {
      setSelectedAlert((current) =>
        current?.id === alert.id ? { ...current, viewedAt: null } : current,
      );
      void Promise.resolve(markUnread(alert.id)).catch((err) => {
        setSourceError(String(err));
        setSelectedAlert((current) =>
          current?.id === alert.id ? { ...current, viewedAt: alert.viewedAt } : current,
        );
      });
    },
    [markUnread],
  );

  const handleDismissSelected = useCallback(
    (alert: AlertRow) => {
      const dismissedAt = Date.now();
      setSelectedAlert((current) =>
        current?.id === alert.id ? { ...current, dismissedAt } : current,
      );
      void Promise.resolve(dismiss(alert.id)).catch((err) => {
        setSourceError(String(err));
        setSelectedAlert((current) =>
          current?.id === alert.id ? { ...current, dismissedAt: alert.dismissedAt } : current,
        );
      });
    },
    [dismiss],
  );

  const handleCheckSources = useCallback(async () => {
    setCheckingSources(true);
    setSourceError(null);
    try {
      await refreshEvents({ projectId });
      await waitForQueuedPolls();
      await refresh();
      publishAlertsChanged(projectId);
      // Polling can add timeline rows without a Rust event, so invalidate Activity.
      publishEventsRecorded(projectId);
    } catch (err) {
      setSourceError(String(err));
    } finally {
      setCheckingSources(false);
    }
  }, [projectId, refresh]);

  return (
    <div className="page-content stack-hero">
      {showDeepLinkNotice ? (
        <DeepLinkNotice
          reason={deepLinkReason ?? CONNECTED_ALERT_UNAVAILABLE}
          availability={connected.feed.availability}
          connectedFailed={connected.failed}
          elsewhere={deepLinkElsewhere}
          truncated={connected.feed.truncated}
          onNavigate={onNavigate}
          onDismiss={() => setDismissedDeepLinkKey(deepLinkKey)}
        />
      ) : null}

      <AlertList
        alerts={displayAlerts}
        filter={filter}
        onFilterChange={handleFilterChange}
        selectedId={selectedAlert?.id ?? null}
        onSelect={handleSelectAlert}
        loading={loading}
        unreadCount={unreadCount}
        onMarkAllRead={markAllRead}
      />

      {selectedAlert ? (
        <AlertDossier
          key={selectedAlert.id}
          alert={selectedAlert}
          onMarkViewed={() => handleMarkViewedSelected(selectedAlert)}
          onMarkUnread={() => handleMarkUnreadSelected(selectedAlert)}
          onDismiss={() => handleDismissSelected(selectedAlert)}
          onNavigate={onNavigate}
          onClose={() => {
            setSelectedAlert(null);
            setSelectedAnchorIndex(null);
          }}
        />
      ) : null}

      <ConnectedAlertTimeline
        feed={connected.feed}
        loading={connected.loading}
        failed={connected.failed}
        destinations={destinationsQuery.data ?? []}
        selectedAlertId={selectedConnectedAlertId}
        onSelect={setSelectedConnectedAlertId}
        onNavigate={onNavigate}
      />

      <AlertSourcesPanel
        checkingSources={checkingSources}
        connectedTypes={connectedTypes}
        expanded={sourcesExpanded}
        error={sourceError ?? error ?? integrationsError}
        loading={integrationsLoading}
        onCheckSources={handleCheckSources}
        onNavigate={onNavigate}
        onToggle={() => setSourcesExpanded((current) => !current)}
      />
    </div>
  );
}

// Never echo untrusted deep-link values in the notice.
function DeepLinkNotice({
  reason,
  availability,
  connectedFailed,
  elsewhere,
  truncated,
  onNavigate,
  onDismiss,
}: {
  reason: string;
  availability: string;
  connectedFailed: boolean;
  elsewhere: ConnectedAlertElsewhere | null;
  truncated: boolean;
  onNavigate?: (page: NavTarget) => void;
  onDismiss: () => void;
}) {
  const outcome = deepLinkOutcome({
    availability,
    connectedFailed,
    elsewhere,
    reason,
    truncated,
  });
  const action = outcome.action;

  return (
    <section className="card" role="status">
      <div className="card__title-rule">
        <span className="card__title">
          <BellOff className="card__icon icon-md" aria-hidden="true" />
          <span>{outcome.headline}</span>
        </span>
        <Button variant="outline" size="sm" onClick={onDismiss}>
          Dismiss
        </Button>
      </div>
      <p className="text-body-muted">{outcome.detail}</p>
      {action && onNavigate ? (
        <Button
          variant="outline"
          size="sm"
          className="deep-link-notice-action"
          onClick={() => onNavigate(action.target)}>
          {action.label}
        </Button>
      ) : null}
    </section>
  );
}

interface DeepLinkOutcome {
  headline: string;
  detail: string;
  action: { label: string; target: NavTarget } | null;
}

// A failed or unavailable read is unknown, not evidence of absence.
function deepLinkOutcome(input: {
  availability: string;
  connectedFailed: boolean;
  elsewhere: ConnectedAlertElsewhere | null;
  reason: string;
  truncated: boolean;
}): DeepLinkOutcome {
  if (input.reason === CONNECTED_LINK_UNKNOWN) {
    return {
      action: null,
      detail:
        "That link points at something this version of SiteCMD has no page for. Updating the app is the fix if it came from a recent message.",
      headline: "Link not recognized",
    };
  }
  if (input.elsewhere) {
    const named = input.elsewhere.projectName;
    return {
      action: { label: "Open Sites", target: "sites" },
      detail: named
        ? `The connected service raised it for ${named}, which is a different project in this app. Switch to it to read the alert.`
        : "The connected service raised it for another site on this account, and no project on this machine is connected to that site. Connecting it here is what makes the alert readable.",
      headline: "That alert is on another site",
    };
  }
  if (input.connectedFailed) {
    return {
      action: null,
      detail:
        "The connected service could not be reached, so whether it still holds that alert is unknown. This is a failed read, not an alert that is gone.",
      headline: "Could not check the connected service",
    };
  }
  if (input.availability === "service_unconfigured") {
    return {
      action: null,
      detail:
        "That alert lives on the SiteCMD service, and this build has no connected service configured, so nothing was ever asked about it. Not finding it here is not evidence that it is gone.",
      headline: "This build has no connected service",
    };
  }
  if (input.availability === "site_not_connected") {
    return {
      action: { label: "Open Sites", target: "sites" },
      detail:
        "This project is not connected to the SiteCMD service, so it has no service alerts to show. The alert belongs to whichever project is.",
      headline: "This project is not connected",
    };
  }
  if (input.availability === "no_installation_token" || input.availability === "not_entitled") {
    return {
      action: { label: "Open Settings", target: "settings:connected" },
      detail:
        "The connected service did not answer for this account, so whether it still holds that alert is unknown. Settings says what it needs.",
      headline: "Could not check the connected service",
    };
  }
  return {
    action: null,
    detail: input.truncated
      ? "That alert is not among the ones this timeline holds. Alerts age out of the connected service after 90 days, and this list covers only the most recent."
      : "That alert is not in this project's timeline. Alerts age out of the connected service after 90 days, and an alert raised for another project lives with that project.",
    headline: "Alert not available",
  };
}

function AlertSourcesPanel({
  checkingSources,
  connectedTypes,
  error,
  expanded,
  loading,
  onCheckSources,
  onNavigate,
  onToggle,
}: {
  checkingSources: boolean;
  connectedTypes: Set<string>;
  error: string | null;
  expanded: boolean;
  loading: boolean;
  onCheckSources: () => void;
  onNavigate?: (page: NavTarget) => void;
  onToggle: () => void;
}) {
  const connectedCount = ALERT_SOURCE_DEFINITIONS.filter((definition) =>
    connectedTypes.has(definition.integrationType),
  ).length;

  return (
    <section className="card">
      <div className="card__title-rule">
        <span className="card__title">
          <BellRing className="card__icon icon-md" aria-hidden="true" />
          <span>Sources</span>
        </span>
        <div className="alert-sources-actions">
          <Button variant="outline" size="sm" onClick={onCheckSources} disabled={checkingSources}>
            <RefreshCw className={`icon-md ${checkingSources ? "animate-spin" : ""}`} />
            {checkingSources ? "Checking" : "Check sources"}
          </Button>
          <Button variant="outline" size="sm" onClick={onToggle}>
            {expanded ? "Hide Sources" : "Show Sources"}
          </Button>
          {onNavigate ? (
            <Button variant="outline" size="sm" onClick={() => onNavigate("integrations")}>
              Open Integrations
            </Button>
          ) : null}
        </div>
      </div>
      {loading ? (
        <LoadingRegion label="Alert sources loading" className="alert-sources-loading">
          <InlineSkeleton className="alert-sources-skeleton" />
        </LoadingRegion>
      ) : (
        <p className="alert-sources-summary">
          {NATIVE_ALERT_DEFINITIONS.length} built-in checks active · {connectedCount} of{" "}
          {ALERT_SOURCE_DEFINITIONS.length} connected services enabled
        </p>
      )}
      {error ? <p className="alert-sources-error">{error}</p> : null}

      {expanded ? (
        <div className="alert-list-grid alert-sources-expanded">
          <div className="alert-source-col alert-source-col--divided">
            <p className="section-label-mid">Built in</p>
            <div className="alert-source-list">
              {NATIVE_ALERT_DEFINITIONS.map((definition) => (
                <NativeSourceRow key={definition.id} definition={definition} />
              ))}
            </div>
          </div>
          <div className="alert-source-col">
            <p className="section-label-mid">Connected services</p>
            <div className="alert-source-list">
              {loading ? (
                <LoadingRegion label="Connected alert services loading" className="stack-base">
                  {[0, 1, 2].map((index) => (
                    <InlineSkeleton key={index} className="alert-source-skeleton" />
                  ))}
                </LoadingRegion>
              ) : (
                ALERT_SOURCE_DEFINITIONS.map((definition) => (
                  <ConnectedSourceRow
                    key={definition.source}
                    definition={definition}
                    connected={connectedTypes.has(definition.integrationType)}
                  />
                ))
              )}
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}

function NativeSourceRow({ definition }: { definition: NativeAlertDefinition }) {
  const Icon = NATIVE_ALERT_ICONS[definition.id] ?? BellRing;

  return (
    <div className="alert-detail-grid">
      <div className="min-w-0">
        <div className="row-tight text-primary">
          <Icon className="icon-sm" />
          <span className="text-body-muted text-strong">{definition.label}</span>
        </div>
        <p className="text-meta alert-source-trigger">{definition.trigger}</p>
      </div>
      <div className="alert-source-meta">
        <p className="section-label-mid text-score-excellent">Built in</p>
        <p className="text-meta alert-source-cadence">{definition.cadence}</p>
      </div>
    </div>
  );
}

function ConnectedSourceRow({
  connected,
  definition,
}: {
  connected: boolean;
  definition: AlertSourceDefinition;
}) {
  const Icon = SOURCE_ICONS[definition.source] ?? BellRing;

  return (
    <div className="alert-detail-grid">
      <div className="min-w-0">
        <div className="row-tight text-primary">
          <Icon className="icon-sm" />
          <span className="text-body-muted text-strong">{definition.label}</span>
        </div>
        <p className="text-meta alert-source-trigger">{definition.trigger}</p>
      </div>
      <div className="alert-source-meta">
        <p className={`section-label-mid ${connected ? "text-score-excellent" : ""}`}>
          {connected ? "Connected" : "Off"}
        </p>
        <p className="text-meta alert-source-cadence">{definition.cadence}</p>
      </div>
    </div>
  );
}

function waitForQueuedPolls(): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, 1500);
  });
}
