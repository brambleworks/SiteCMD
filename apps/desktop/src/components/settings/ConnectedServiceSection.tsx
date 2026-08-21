import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  ConnectedErasureReceipt,
  ConnectedSiteChallenge,
} from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { useToast } from "@/hooks/useToast";
import { copyToClipboard } from "@/lib/clipboard";
import {
  fetchConnectedSiteState,
  getConnectedStatus,
  inspectConnectedSync,
  syncConnectedSite,
} from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import { AccountRecoveryCard } from "./AccountRecoveryCard";
import { ConnectedAlertDestinationsSection } from "./ConnectedAlertDestinationsSection";
import { ConnectedServiceManagement } from "./ConnectedServiceManagement";
import { ConnectedServiceSetup } from "./ConnectedServiceSetup";
import { ProviderConnectionsSection } from "./ProviderConnectionsSection";
import { SiteOwnershipCard } from "./SiteOwnershipCard";

interface ConnectedServiceSectionProps {
  projectId?: number;
  environmentScopeKey?: string;
}

function connectedStandingLabel(overPlan: boolean, graceExpiresAt: string | null): string {
  if (!overPlan) return "Within plan";
  if (!graceExpiresAt) return "Over plan";
  const deadline = new Date(graceExpiresAt);
  if (!Number.isFinite(deadline.getTime())) return "Over plan";
  const label = deadline.toLocaleString();
  return deadline.getTime() <= Date.now() ? `Suspended since ${label}` : `Grace until ${label}`;
}

export function ConnectedServiceSection({
  projectId,
  environmentScopeKey,
}: ConnectedServiceSectionProps) {
  const toast = useToast();
  const queryClient = useQueryClient();
  const canLoad = projectId != null && Boolean(environmentScopeKey);
  const activeScopeKey = `${projectId ?? ""}\u0000${environmentScopeKey ?? ""}`;
  const queryKey = queryKeys.settings.connectedStatus(projectId ?? 0, environmentScopeKey ?? "");
  const remoteKey = queryKeys.settings.connectedRemoteState(
    projectId ?? 0,
    environmentScopeKey ?? "",
  );
  const statusQuery = useQuery({
    queryKey,
    queryFn: () =>
      getConnectedStatus({
        projectId: projectId as number,
        environmentScopeKey: environmentScopeKey as string,
      }),
    enabled: canLoad,
  });
  const remoteQuery = useQuery({
    queryKey: remoteKey,
    queryFn: () =>
      fetchConnectedSiteState({
        projectId: projectId as number,
        environmentScopeKey: environmentScopeKey as string,
      }),
    enabled: canLoad && statusQuery.data?.connected === true,
  });
  const [payload, setPayload] = useState("");
  const [inspecting, setInspecting] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [challenge, setChallenge] = useState<ConnectedSiteChallenge | null>(null);
  const [scopedErasureReceipt, setScopedErasureReceipt] = useState<{
    scopeKey: string;
    receipt: ConnectedErasureReceipt;
  } | null>(null);
  const erasureReceipt =
    scopedErasureReceipt?.scopeKey === activeScopeKey ? scopedErasureReceipt.receipt : null;

  if (!canLoad) {
    return (
      <section className="card card--spacious">
        <p className="body-muted">Select a project environment to manage its connection.</p>
      </section>
    );
  }

  const scope = {
    projectId: projectId as number,
    environmentScopeKey: environmentScopeKey as string,
  };
  const refreshStatus = async () => {
    await queryClient.invalidateQueries({ queryKey });
  };
  const refreshConnection = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey }),
      queryClient.invalidateQueries({ queryKey: remoteKey }),
    ]);
  };
  const resetConnection = async () => {
    setPayload("");
    await refreshConnection();
  };

  const handleInspect = async () => {
    setInspecting(true);
    try {
      const inspection = await inspectConnectedSync(scope);
      setPayload(inspection.payload);
    } catch (error) {
      toast.error("Payload inspection failed", String(error));
    } finally {
      setInspecting(false);
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    try {
      const result = await syncConnectedSite(scope);
      await refreshConnection();
      if (result.keyRotationCompleted !== null) {
        toast.success(
          `Key rotation complete: version ${result.keyRotationCompleted} is now in force`,
          "Mint a new CI token key for your pipeline; the old key stops being accepted.",
        );
      }
      const settled = `Submission ${result.submissionSequence} accepted; ${result.mutationsSettled} local change${result.mutationsSettled === 1 ? "" : "s"} settled.`;
      toast.success(
        "Connected state synced",
        result.scopeDeliveryPending
          ? `${settled} The scan scope has not been delivered yet and retries in the background.`
          : settled,
      );
    } catch (error) {
      toast.error("Connected sync failed", String(error));
    } finally {
      setSyncing(false);
    }
  };

  const handleVerified = async () => {
    setChallenge(null);
    await refreshConnection();
  };

  if (statusQuery.isPending) {
    return (
      <LoadingRegion label="Connected service loading state" className="card card--spacious">
        <Skeleton className="connected-skeleton-title" />
        <Skeleton className="connected-skeleton-line" />
      </LoadingRegion>
    );
  }

  if (statusQuery.isError || !statusQuery.data) {
    return (
      <section className="card card--spacious" role="alert">
        <h2 className="settings-card-title">Connected Service</h2>
        <p className="agent-handoff-error">Connection status could not load.</p>
        <Button variant="outline" size="sm" onClick={() => void statusQuery.refetch()}>
          Retry
        </Button>
      </section>
    );
  }

  const status = statusQuery.data;
  const pendingChallenge = challenge ?? remoteQuery.data?.challenge ?? null;
  const disconnected = remoteQuery.data?.phase === "disconnected";
  const remoteActive = status.connected && !disconnected;

  return (
    <div className="settings-section-stack">
      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Connection</h2>
          <span className={`tone-badge ${remoteActive ? "tone-badge--low" : "tone-badge--muted"}`}>
            {disconnected ? "Stopped" : status.connected ? "Connected" : "Local only"}
          </span>
        </div>
        <p className="body-muted">
          {status.connected
            ? "SiteCMD can sync inspected findings and lifecycle changes for this production environment."
            : "Nothing leaves this desktop until a site connection is imported and you approve a sync."}
        </p>
        <dl className="connected-facts">
          <div>
            <dt>Environment</dt>
            <dd>{environmentScopeKey}</dd>
          </div>
          <div>
            <dt>Site ID</dt>
            <dd>{status.siteId ?? "Not assigned"}</dd>
          </div>
          <div>
            <dt>Bootstrap</dt>
            <dd>{status.bootstrapped ? "Committed" : "Not sent"}</dd>
          </div>
          <div>
            <dt>Outbox</dt>
            <dd>
              {status.pendingMutations} pending
              {status.conflictedMutations > 0
                ? `, ${status.conflictedMutations} need reconciliation`
                : ""}
            </dd>
          </div>
          <div>
            <dt>Scope delivery</dt>
            <dd>{status.pendingScopeSync ? "Retry pending" : "Up to date"}</dd>
          </div>
          {remoteQuery.data ? (
            <>
              <div>
                <dt>Hosted scope</dt>
                <dd>
                  {remoteQuery.data.scopeEffectiveRouteCount ??
                    remoteQuery.data.scopeRoutes?.length ??
                    0}{" "}
                  routes
                  {remoteQuery.data.scopeOverPlan
                    ? `, ${remoteQuery.data.scopeOverflowCount} over ${remoteQuery.data.scopeRouteCap}-route plan, ${connectedStandingLabel(
                        true,
                        remoteQuery.data.scopeOverPlanGraceExpiresAt,
                      )}`
                    : ""}
                </dd>
              </div>
              <div>
                <dt>Site allowance</dt>
                <dd>
                  {connectedStandingLabel(
                    remoteQuery.data.siteAllowanceOverPlan ?? false,
                    remoteQuery.data.siteAllowanceOverPlanGraceExpiresAt ?? null,
                  )}
                </dd>
              </div>
            </>
          ) : null}
        </dl>
        {!status.endpointConfigured ? (
          <p className="agent-handoff-error" role="alert">
            This build has no connected-service endpoint configured, so inspection works but sync is
            unavailable.
          </p>
        ) : null}
        <div className="connected-actions">
          <Button variant="outline" onClick={() => void handleInspect()} disabled={inspecting}>
            {inspecting ? "Inspecting..." : "Inspect Payload"}
          </Button>
          {remoteActive ? (
            <Button
              onClick={() => void handleSync()}
              disabled={
                syncing ||
                !status.endpointConfigured ||
                !status.hasInstallationToken ||
                !status.hasFingerprintKey
              }>
              {syncing ? "Syncing..." : "Sync Now"}
            </Button>
          ) : null}
        </div>
        {payload ? (
          <div className="connected-payload-wrap">
            <div className="row-between">
              <p className="text-13-medium">
                {status.connected ? "Current payload preview" : "Payload shape preview"}
              </p>
              <Button size="sm" variant="ghost" onClick={() => void copyToClipboard(payload)}>
                Copy JSON
              </Button>
            </div>
            <pre className="connected-payload">{payload}</pre>
            <p className="text-body-muted">
              Nothing has been sent. Sync uses this exact public wire schema and serializer.
            </p>
          </div>
        ) : null}
      </section>

      {pendingChallenge ? (
        <SiteOwnershipCard
          projectId={scope.projectId}
          environmentScopeKey={scope.environmentScopeKey}
          challenge={pendingChallenge}
          onVerified={handleVerified}
        />
      ) : null}

      {erasureReceipt ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Erasure Receipt</h2>
          </div>
          <div className="connected-payload-wrap">
            <div className="row-between">
              <p className="text-13-medium">Copy this now</p>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void copyToClipboard(erasureReceipt.statusToken)}>
                Copy Receipt Token
              </Button>
            </div>
            <pre className="connected-payload">{erasureReceipt.statusToken}</pre>
            <p className="text-body-muted">
              Job {erasureReceipt.jobId}. This token is the only later proof the data was erased,
              and it is readable exactly once here.
            </p>
          </div>
        </section>
      ) : null}

      {!status.connected ? (
        <ConnectedServiceSetup
          scope={scope}
          status={status}
          onChallenge={setChallenge}
          onStatusChanged={refreshStatus}
        />
      ) : (
        <ConnectedServiceManagement
          scope={scope}
          status={status}
          disconnected={disconnected}
          onConnectionChanged={refreshConnection}
          onConnectionReset={resetConnection}
          onErased={(receipt) => setScopedErasureReceipt({ scopeKey: activeScopeKey, receipt })}
        />
      )}
      <ProviderConnectionsSection />
      {status.hasInstallationToken ? <ConnectedAlertDestinationsSection /> : null}
      {status.hasInstallationToken ? <AccountRecoveryCard /> : null}
    </div>
  );
}
