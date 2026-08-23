import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { ConnectedSiteCredential } from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { copyToClipboard } from "@/lib/clipboard";
import {
  listConnectedSiteCredentials,
  mintConnectedWebhookSecret,
  revokeConnectedSiteCredential,
  rotateConnectedSiteCredential,
} from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import { userFacingError } from "@/lib/user-facing-error";

interface ConnectedCredentialsSectionProps {
  projectId: number;
  environmentScopeKey: string;
}

/** A shown-once webhook secret, with the overlap deadline when a rotation
 *  produced it. */
interface RevealedSecret {
  tokenId: string;
  secret: string;
  rotationOverlapUntil: string | null;
}

function credentialLabel(credential: ConnectedSiteCredential): string {
  if (credential.kind === "webhook") {
    return `Deploy webhook secret, generation ${credential.secretGeneration ?? 1}`;
  }
  return credential.repository ? `CI token for ${credential.repository}` : "CI token";
}

function credentialDetail(credential: ConnectedSiteCredential): string {
  if (credential.revokedAt) {
    return `revoked ${credential.revokedAt.slice(0, 10)}`;
  }
  const parts: string[] = [];
  if (credential.secretFingerprint) parts.push(credential.secretFingerprint);
  if (credential.kind === "ci") {
    parts.push(
      credential.lastUsedAt ? `last used ${credential.lastUsedAt.slice(0, 10)}` : "never used yet",
    );
  }
  if (credential.rotationOverlapUntil) {
    parts.push("rotation overlap active, both generations open the door");
  }
  return parts.join("; ");
}

/** Lists credential fingerprints and tombstones; secret values appear only at creation. */
export function ConnectedCredentialsSection({
  projectId,
  environmentScopeKey,
}: ConnectedCredentialsSectionProps) {
  const toast = useToast();
  const queryClient = useQueryClient();
  const queryKey = queryKeys.settings.connectedSiteCredentials(projectId, environmentScopeKey);
  const scope = { environmentScopeKey, projectId };
  const credentialsQuery = useQuery({
    queryKey,
    queryFn: () => listConnectedSiteCredentials(scope),
  });
  const [revealed, setRevealed] = useState<RevealedSecret | null>(null);
  const [busyCredential, setBusyCredential] = useState<string | null>(null);
  const [minting, setMinting] = useState(false);

  const refresh = () => queryClient.invalidateQueries({ queryKey });

  const handleMint = async () => {
    setMinting(true);
    try {
      const minted = await mintConnectedWebhookSecret(scope);
      setRevealed({
        rotationOverlapUntil: null,
        secret: minted.secret,
        tokenId: minted.id,
      });
      await refresh();
      toast.success("Webhook secret minted", "Copy it now. It is not shown again.");
    } catch (error) {
      toast.error(
        "Could not mint the webhook secret",
        userFacingError(error, "Your change was not saved. Try again."),
      );
    } finally {
      setMinting(false);
    }
  };

  const handleRotate = async (tokenId: string) => {
    setBusyCredential(tokenId);
    try {
      const rotated = await rotateConnectedSiteCredential({ ...scope, tokenId });
      setRevealed({
        rotationOverlapUntil: rotated.rotationOverlapUntil,
        secret: rotated.secret,
        tokenId: rotated.id,
      });
      await refresh();
      toast.success("Secret rotated", "Copy the new secret now. It is not shown again.");
    } catch (error) {
      toast.error(
        "Could not rotate the secret",
        userFacingError(error, "Your change was not saved. Try again."),
      );
    } finally {
      setBusyCredential(null);
    }
  };

  const handleRevoke = async (tokenId: string) => {
    setBusyCredential(tokenId);
    try {
      await revokeConnectedSiteCredential({ ...scope, tokenId });
      if (revealed?.tokenId === tokenId) setRevealed(null);
      await refresh();
      toast.success("Credential revoked");
    } catch (error) {
      toast.error(
        "Could not revoke the credential",
        userFacingError(error, "Your change was not saved. Try again."),
      );
    } finally {
      setBusyCredential(null);
    }
  };

  const credentials = credentialsQuery.data ?? [];
  const webhookSecret = credentials.find((credential) => credential.kind === "webhook");
  const liveWebhookSecret = webhookSecret && !webhookSecret.revokedAt ? webhookSecret : null;

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Site Credentials</h2>
      </div>
      <p className="body-muted">
        Every machine credential this site holds. CI tokens are created above and revoked here; the
        deploy webhook secret signs deploy reports from pipelines outside the connected providers,
        posted to the site's deploy hook. One webhook secret exists per site: rotating it keeps the
        previous generation working for 24 hours so your pipeline switches without a gap.
      </p>
      {credentialsQuery.isError ? (
        <p className="agent-handoff-error" role="alert">
          Site credentials could not load.
        </p>
      ) : null}
      {credentials.length > 0 ? (
        <div className="webhook-list">
          {credentials.map((credential) => (
            <div key={credential.id} className="settings-webhook-row">
              <span
                className={
                  credential.revokedAt ? "status-dot-info status-dot-dim" : "status-dot-success"
                }
              />
              <div className="flex-fill min-w-0">
                <div className="text-mono-sm text-truncate">{credentialLabel(credential)}</div>
                <div className="text-13-muted">
                  {credential.id}
                  {credentialDetail(credential) ? `; ${credentialDetail(credential)}` : ""}
                </div>
              </div>
              {credential.kind === "webhook" && !credential.revokedAt ? (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleRotate(credential.id)}
                  disabled={busyCredential === credential.id}>
                  Rotate Secret
                </Button>
              ) : null}
              {!credential.revokedAt ? (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleRevoke(credential.id)}
                  disabled={busyCredential === credential.id}>
                  Revoke
                </Button>
              ) : null}
            </div>
          ))}
        </div>
      ) : credentialsQuery.isSuccess ? (
        <p className="body-muted">No credentials yet.</p>
      ) : null}
      {credentialsQuery.isSuccess && !liveWebhookSecret ? (
        <div className="stack-base connected-form">
          <Button variant="outline" onClick={() => void handleMint()} disabled={minting}>
            {minting
              ? "Minting..."
              : webhookSecret
                ? "Mint Webhook Secret Again"
                : "Mint Webhook Secret"}
          </Button>
          <p className="text-body-muted">
            For deploy pipelines outside the connected providers: your CI signs each deploy report
            with this secret and posts it to the site's deploy hook.
          </p>
        </div>
      ) : null}
      {revealed ? (
        <div className="connected-payload-wrap">
          <div className="row-between">
            <p className="text-13-medium">Webhook secret (copy this now)</p>
            <Button size="sm" variant="ghost" onClick={() => void copyToClipboard(revealed.secret)}>
              Copy Secret
            </Button>
          </div>
          <pre className="connected-payload">{revealed.secret}</pre>
          <p className="text-body-muted">
            {revealed.rotationOverlapUntil
              ? "Update the secret in your pipeline. For 24 hours the previous generation still opens the deploy hook, so an in-flight deploy is not refused mid-switch."
              : "Sign each delivery sha256 over the timestamp, a dot, and the raw body, sent as X-SiteCMD-Signature with the timestamp in X-SiteCMD-Timestamp. SiteCMD stores only a fingerprint, so this is the one time it is readable."}
          </p>
        </div>
      ) : null}
    </section>
  );
}
