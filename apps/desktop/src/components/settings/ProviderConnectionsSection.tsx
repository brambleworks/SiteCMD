import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { ConnectedCreatedProviderConnection } from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import {
  createConnectedProviderConnection,
  listConnectedProviderConnections,
  revokeConnectedProviderConnection,
} from "@/lib/commands";
import { openExternalUrl } from "@/lib/commands/desktop";
import { queryKeys } from "@/lib/query/query-keys";

const PROVIDER_LABELS: Record<string, string> = { netlify: "Netlify", vercel: "Vercel" };

function providerLabel(provider: string): string {
  return PROVIDER_LABELS[provider] ?? provider;
}

/** Manage browser-completed provider OAuth links; credentials never reach the desktop. */
export function ProviderConnectionsSection() {
  const toast = useToast();
  const queryClient = useQueryClient();
  const queryKey = queryKeys.settings.connectedProviderConnections();
  const connectionsQuery = useQuery({
    queryKey,
    queryFn: () => listConnectedProviderConnections(),
  });
  const [starting, setStarting] = useState<string | null>(null);
  const [busyConnection, setBusyConnection] = useState<string | null>(null);
  const [pendingRound, setPendingRound] = useState<ConnectedCreatedProviderConnection | null>(null);

  const refresh = () => queryClient.invalidateQueries({ queryKey });

  const handleConnect = async (provider: "vercel" | "netlify") => {
    setStarting(provider);
    try {
      const round = await createConnectedProviderConnection({ provider });
      setPendingRound(round);
      await refresh();
    } catch (error) {
      toast.error("Could not start the provider connection", String(error));
    } finally {
      setStarting(null);
    }
  };

  const handleOpenAuthorize = async () => {
    if (!pendingRound) return;
    try {
      await openExternalUrl({ url: pendingRound.authorizeUrl });
      toast.success(
        "Finish in your browser",
        "Approve the request there, then refresh this list to see the connection go active.",
      );
    } catch (error) {
      toast.error("Could not open the provider sign-in", String(error));
    }
  };

  const handleRevoke = async (connectionId: string) => {
    setBusyConnection(connectionId);
    try {
      await revokeConnectedProviderConnection({ connectionId });
      if (pendingRound?.connection.id === connectionId) setPendingRound(null);
      await refresh();
      toast.success("Provider connection revoked");
    } catch (error) {
      toast.error("Could not revoke the connection", String(error));
    } finally {
      setBusyConnection(null);
    }
  };

  const connections = connectionsQuery.data ?? [];

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Provider Connections</h2>
      </div>
      <p className="body-muted">
        Link your Vercel or Netlify account once and it works for every site on the plan: the
        provider's own records prove a project serves your domain, and deploys are reported
        automatically through a webhook SiteCMD provisions on the bound project. The provider
        credential lives encrypted on the service; this desktop never holds it.
      </p>
      {connectionsQuery.isError ? (
        <p className="agent-handoff-error" role="alert">
          Provider connections could not load. Only admin installations can manage them.
        </p>
      ) : null}
      {connections.length > 0 ? (
        <div className="webhook-list">
          {connections.map((connection) => (
            <div key={connection.id} className="settings-webhook-row">
              <span
                className={
                  connection.status === "active"
                    ? "status-dot-success"
                    : "status-dot-info status-dot-dim"
                }
              />
              <div className="flex-fill min-w-0">
                <div className="text-mono-sm text-truncate">
                  {providerLabel(connection.provider)}
                  {connection.externalAccount
                    ? ` - ${connection.externalAccount.name ?? connection.externalAccount.id}`
                    : ""}
                </div>
                <div className="text-13-muted">
                  {connection.status}
                  {connection.failedReason ? `: ${connection.failedReason}` : ""}
                  {connection.revokedReason ? `: ${connection.revokedReason}` : ""}
                  {connection.grantedScopes ? `; granted ${connection.grantedScopes}` : ""}
                </div>
              </div>
              {connection.status === "active" || connection.status === "pending" ? (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleRevoke(connection.id)}
                  disabled={busyConnection === connection.id}>
                  Revoke
                </Button>
              ) : null}
            </div>
          ))}
        </div>
      ) : connectionsQuery.isSuccess ? (
        <p className="body-muted">No provider connections yet.</p>
      ) : null}
      <div className="connected-actions">
        <Button
          variant="outline"
          onClick={() => void handleConnect("vercel")}
          disabled={starting !== null}>
          {starting === "vercel" ? "Starting..." : "Connect Vercel"}
        </Button>
        <Button
          variant="outline"
          onClick={() => void handleConnect("netlify")}
          disabled={starting !== null}>
          {starting === "netlify" ? "Starting..." : "Connect Netlify"}
        </Button>
        <Button variant="ghost" onClick={() => void refresh()}>
          Refresh
        </Button>
      </div>
      {pendingRound ? (
        <div className="connected-payload-wrap">
          <p className="text-13-medium">
            {providerLabel(pendingRound.connection.provider)} will be asked for:
          </p>
          <pre className="connected-payload">{pendingRound.requestedScopes}</pre>
          <p className="text-body-muted">
            Approving in the browser hands the credential to the service, never to this desktop. The
            link expires if unused; starting over is free.
          </p>
          <Button onClick={() => void handleOpenAuthorize()}>Open Provider Sign-in</Button>
        </div>
      ) : null}
    </section>
  );
}
