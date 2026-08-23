import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ConnectedSiteChallenge } from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { copyToClipboard } from "@/lib/clipboard";
import {
  listConnectedProviderConnections,
  listConnectedProviderProjects,
  verifyConnectedSite,
  verifyConnectedSiteProvider,
} from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import { userFacingError } from "@/lib/user-facing-error";

interface SiteOwnershipCardProps {
  projectId: number;
  environmentScopeKey: string;
  challenge: ConnectedSiteChallenge;
  /** Refresh ownership and remote state after verification. */
  onVerified: () => Promise<void>;
}

/** Site ownership methods shown until verification completes. */
export function SiteOwnershipCard({
  projectId,
  environmentScopeKey,
  challenge,
  onVerified,
}: SiteOwnershipCardProps) {
  const toast = useToast();
  const scope = { environmentScopeKey, projectId };
  const [verifying, setVerifying] = useState(false);
  const [providerConnectionId, setProviderConnectionId] = useState("");
  const [providerProjectId, setProviderProjectId] = useState("");
  // The provider-attested path's ingredients: an active connection to pick,
  // then its projects. Mounted only while a site still needs proving.
  const providerConnectionsQuery = useQuery({
    queryKey: queryKeys.settings.connectedProviderConnections(),
    queryFn: () => listConnectedProviderConnections(),
  });
  const providerProjectsQuery = useQuery({
    queryKey: queryKeys.settings.connectedProviderProjects(providerConnectionId),
    queryFn: () => listConnectedProviderProjects({ connectionId: providerConnectionId }),
    enabled: providerConnectionId !== "",
  });

  const handleVerify = async (method: "dns_txt" | "well_known") => {
    setVerifying(true);
    try {
      const result = await verifyConnectedSite({ ...scope, method });
      if (result.verified) {
        await onVerified();
        toast.success("Ownership proved", "Sync now to import this environment's issue list.");
      } else {
        toast.error("Not verified yet", "The challenge was not found where this method looks.");
      }
    } catch (error) {
      toast.error(
        "Verification failed",
        userFacingError(error, "Run the verification again after the site has deployed."),
      );
    } finally {
      setVerifying(false);
    }
  };

  const handleProviderVerify = async () => {
    setVerifying(true);
    try {
      const result = await verifyConnectedSiteProvider({
        ...scope,
        connectionId: providerConnectionId,
        externalProjectId: providerProjectId,
      });
      if (result.verified) {
        await onVerified();
        toast.success(
          "Ownership proved through the provider",
          result.deployTriggerStatus === "provisioned"
            ? "Deploys will be reported automatically. Sync now to import this environment's issue list."
            : "Sync now to import this environment's issue list.",
        );
      }
    } catch (error) {
      toast.error(
        "Provider verification failed",
        userFacingError(error, "Run the verification again after the site has deployed."),
      );
    } finally {
      setVerifying(false);
    }
  };

  const activeConnections = (providerConnectionsQuery.data ?? []).filter(
    (connection) => connection.status === "active",
  );

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Prove You Own This Domain</h2>
      </div>
      <p className="body-muted">
        Publish either proof, then verify. Nothing is scanned and no mail is sent until one of them
        is found, which is what stops anyone pointing SiteCMD at a domain they do not own.
      </p>
      <dl className="connected-facts">
        <div>
          <dt>DNS record</dt>
          <dd>
            {challenge.dnsType} {challenge.dnsName}
          </dd>
        </div>
        <div>
          <dt>Or file at</dt>
          <dd>{challenge.wellKnownPath}</dd>
        </div>
      </dl>
      <div className="connected-payload-wrap">
        <div className="row-between">
          <p className="text-13-medium">Value to publish</p>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void copyToClipboard(challenge.challenge)}>
            Copy Value
          </Button>
        </div>
        <pre className="connected-payload">{challenge.challenge}</pre>
      </div>
      <div className="connected-actions">
        <Button onClick={() => void handleVerify("dns_txt")} disabled={verifying}>
          {verifying ? "Checking..." : "Verify DNS Record"}
        </Button>
        <Button
          variant="outline"
          onClick={() => void handleVerify("well_known")}
          disabled={verifying}>
          Verify Well-Known File
        </Button>
      </div>
      {activeConnections.length > 0 ? (
        <div className="stack-base connected-form">
          <label className="form-label" htmlFor="connected-provider-connection">
            Or let a connected provider vouch for it: its own records prove the project serves this
            domain, and deploy reporting is set up in the same step.
          </label>
          <select
            id="connected-provider-connection"
            className="field-control"
            value={providerConnectionId}
            onChange={(event) => {
              setProviderConnectionId(event.target.value);
              setProviderProjectId("");
            }}>
            <option value="">Choose a provider connection</option>
            {activeConnections.map((connection) => (
              <option key={connection.id} value={connection.id}>
                {connection.provider}
                {connection.externalAccount
                  ? ` - ${connection.externalAccount.name ?? connection.externalAccount.id}`
                  : ""}
              </option>
            ))}
          </select>
          {providerConnectionId ? (
            <select
              className="field-control"
              aria-label="Provider project"
              value={providerProjectId}
              onChange={(event) => setProviderProjectId(event.target.value)}>
              <option value="">
                {providerProjectsQuery.isPending
                  ? "Loading projects..."
                  : "Choose the project serving this domain"}
              </option>
              {(providerProjectsQuery.data ?? []).map((project) => (
                <option key={project.externalProjectId} value={project.externalProjectId}>
                  {project.name}
                </option>
              ))}
            </select>
          ) : null}
          <Button
            variant="outline"
            onClick={() => void handleProviderVerify()}
            disabled={verifying || !providerConnectionId || !providerProjectId}>
            {verifying ? "Checking..." : "Verify Through Provider"}
          </Button>
        </div>
      ) : null}
    </section>
  );
}
