import { useState } from "react";
import type {
  ConnectedCiToken,
  ConnectedErasureReceipt,
  ConnectedRemintedSecret,
  ConnectedStatus,
} from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useToast } from "@/hooks/useToast";
import { copyToClipboard } from "@/lib/clipboard";
import {
  disconnectConnectedSite,
  eraseConnectedSite,
  exportConnectedConnection,
  mintConnectedCiToken,
  reconnectConnectedSite,
  unlinkConnectedSite,
  type ConnectedScopeArgs,
} from "@/lib/commands";
import { ConnectedAlertWebhooksSection } from "./ConnectedAlertWebhooksSection";
import { ConnectedCredentialsSection } from "./ConnectedCredentialsSection";
import { ConnectedNotificationSettingsSection } from "./ConnectedNotificationSettingsSection";
import { ConnectedReportsSection } from "./ConnectedReportsSection";
import { KeyRotationCard } from "./KeyRotationCard";
import { userFacingError } from "@/lib/user-facing-error";

interface ConnectedServiceManagementProps {
  scope: ConnectedScopeArgs;
  status: ConnectedStatus;
  disconnected: boolean;
  onConnectionChanged: () => Promise<void>;
  onConnectionReset: () => Promise<void>;
  onErased: (receipt: ConnectedErasureReceipt) => void;
}

export function ConnectedServiceManagement({
  scope,
  status,
  disconnected,
  onConnectionChanged,
  onConnectionReset,
  onErased,
}: ConnectedServiceManagementProps) {
  const toast = useToast();
  const [exportPassphrase, setExportPassphrase] = useState("");
  const [encryptedExport, setEncryptedExport] = useState("");
  const [exporting, setExporting] = useState(false);
  const [ciRepository, setCiRepository] = useState("");
  const [ciWorkflowRef, setCiWorkflowRef] = useState("");
  const [ciGitRef, setCiGitRef] = useState("");
  const [ciToken, setCiToken] = useState<ConnectedCiToken | null>(null);
  const [minting, setMinting] = useState(false);
  const [unlinking, setUnlinking] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const [reconnecting, setReconnecting] = useState(false);
  const [remintedSecret, setRemintedSecret] = useState<ConnectedRemintedSecret | null>(null);
  const [erasing, setErasing] = useState(false);

  const handleExport = async () => {
    setExporting(true);
    try {
      const encrypted = await exportConnectedConnection({
        ...scope,
        passphrase: exportPassphrase,
      });
      setEncryptedExport(encrypted);
      setExportPassphrase("");
      toast.success("Encrypted connection created");
    } catch (error) {
      toast.error(
        "Connection export failed",
        userFacingError(error, "Nothing was written. Try again."),
      );
    } finally {
      setExporting(false);
    }
  };

  const handleCopyExport = async () => {
    if (await copyToClipboard(encryptedExport)) {
      toast.success("Encrypted connection copied");
    } else {
      toast.error("Could not copy encrypted connection");
    }
  };

  const handleMintCiToken = async () => {
    setMinting(true);
    try {
      setCiToken(
        await mintConnectedCiToken({
          ...scope,
          repository: ciRepository.trim(),
          workflowRef: ciWorkflowRef.trim(),
          gitRef: ciGitRef.trim(),
        }),
      );
      toast.success("CI token created", "Copy it now. It is not recoverable.");
    } catch (error) {
      toast.error(
        "Could not create a CI token",
        userFacingError(error, "Your change was not saved. Try again."),
      );
    } finally {
      setMinting(false);
    }
  };

  const handleUnlink = async () => {
    setUnlinking(true);
    try {
      await unlinkConnectedSite(scope);
      setEncryptedExport("");
      await onConnectionReset();
      toast.success("Site unlinked", "The local fingerprint key was removed from this desktop.");
    } catch (error) {
      toast.error(
        "Could not unlink site",
        userFacingError(error, "Your change was not saved. Try again."),
      );
    } finally {
      setUnlinking(false);
    }
  };

  const handleDisconnect = async () => {
    setDisconnecting(true);
    try {
      await disconnectConnectedSite(scope);
      setEncryptedExport("");
      await onConnectionReset();
      toast.success(
        "Site disconnected",
        "The service stopped watching it. Its data is kept for 30 days if you reconnect.",
      );
    } catch (error) {
      toast.error(
        "Could not disconnect the site",
        userFacingError(error, "Your change was not saved. Try again."),
      );
    } finally {
      setDisconnecting(false);
    }
  };

  const handleReconnect = async () => {
    setReconnecting(true);
    try {
      const resumed = await reconnectConnectedSite(scope);
      setRemintedSecret(resumed.webhookSecret ?? null);
      await onConnectionChanged();
      toast.success(
        "Site resumed",
        resumed.webhookSecret
          ? "The service is watching again. Copy the reminted webhook secret; it is not shown again."
          : "The service is watching this site again.",
      );
    } catch (error) {
      toast.error("Could not resume the site", userFacingError(error, "Try again in a moment."));
    } finally {
      setReconnecting(false);
    }
  };

  const handleErase = async () => {
    setErasing(true);
    try {
      const receipt = await eraseConnectedSite(scope);
      onErased(receipt);
      setEncryptedExport("");
      await onConnectionReset();
      toast.success("Site data erased", "Copy the receipt token now. It is not recoverable.");
    } catch (error) {
      toast.error(
        "Could not erase the site",
        userFacingError(error, "Your change was not saved. Try again."),
      );
    } finally {
      setErasing(false);
    }
  };

  return (
    <>
      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Transfer Connection</h2>
        </div>
        <p className="body-muted">
          The encrypted export contains site metadata and the fingerprint key. It never contains the
          installation token. Use a passphrase of at least 12 characters and transfer the token
          separately.
        </p>
        <div className="stack-base connected-form">
          <label className="form-label" htmlFor="connected-export-passphrase">
            Export passphrase
          </label>
          <Input
            id="connected-export-passphrase"
            type="password"
            autoComplete="new-password"
            value={exportPassphrase}
            onChange={(event) => setExportPassphrase(event.target.value)}
          />
          <Button
            variant="outline"
            onClick={() => void handleExport()}
            disabled={exporting || exportPassphrase.length < 12}>
            {exporting ? "Encrypting..." : "Create Encrypted Export"}
          </Button>
        </div>
        {encryptedExport ? (
          <div className="connected-payload-wrap">
            <pre className="connected-payload">{encryptedExport}</pre>
            <Button size="sm" variant="outline" onClick={() => void handleCopyExport()}>
              Copy Encrypted Export
            </Button>
          </div>
        ) : null}
      </section>

      {status.bootstrapped && !disconnected ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">CI Gate Credential</h2>
          </div>
          <p className="body-muted">
            A token for this site alone. It can ask whether a branch introduces findings the
            baseline does not have, read only the deployment-ordering cursor, record deployments,
            and submit code evidence from a checkout. It cannot read findings, lifecycle state, or
            account data, because a CI secret is readable by anyone who can edit a workflow file.
          </p>
          <div className="stack-base connected-form">
            <label className="form-label" htmlFor="connected-ci-repository">
              Repository (optional)
            </label>
            <Input
              id="connected-ci-repository"
              autoComplete="off"
              placeholder="owner/repo"
              value={ciRepository}
              onChange={(event) => setCiRepository(event.target.value)}
            />
            <label className="form-label" htmlFor="connected-ci-workflow">
              Trusted workflow (required for verified CI)
            </label>
            <Input
              id="connected-ci-workflow"
              autoComplete="off"
              placeholder=".github/workflows/sitecmd.yml"
              value={ciWorkflowRef}
              onChange={(event) => setCiWorkflowRef(event.target.value)}
            />
            <label className="form-label" htmlFor="connected-ci-ref">
              Trusted ref (optional)
            </label>
            <Input
              id="connected-ci-ref"
              autoComplete="off"
              placeholder="refs/heads/main"
              value={ciGitRef}
              onChange={(event) => setCiGitRef(event.target.value)}
            />
            <Button
              variant="outline"
              onClick={() => void handleMintCiToken()}
              disabled={minting || !status.endpointConfigured}>
              {minting ? "Creating..." : "Create CI Token"}
            </Button>
          </div>
          {ciToken ? (
            <div className="connected-payload-wrap">
              <div className="row-between">
                <p className="text-13-medium">Copy this now</p>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void copyToClipboard(ciToken.token)}>
                  Copy Token
                </Button>
              </div>
              <pre className="connected-payload">{ciToken.token}</pre>
              {ciToken.repositoryId ? (
                <div className="stack-tight">
                  <p className="text-body-muted">
                    Stored only as a hash, so this is the one time it is readable. It is pinned to
                    immutable GitHub repository id {ciToken.repositoryId}. Set it as
                    SITECMD_CI_TOKEN, grant the job <code>id-token: write</code>, and run{" "}
                    <code>sitecmd connected --submit</code> to send verified CI evidence.
                  </p>
                  {ciToken.orderingAuthorityId && ciToken.orderingAuthorityEpoch !== null ? (
                    <p className="text-body-muted">
                      Governing publish authority selected: {ciToken.orderingAuthorityId}, epoch{" "}
                      {ciToken.orderingAuthorityEpoch}.
                    </p>
                  ) : null}
                </div>
              ) : (
                <p className="text-body-muted">
                  Stored only as a hash, so this is the one time it is readable. Set it as
                  SITECMD_CI_TOKEN and run <code>sitecmd gate</code> in your workflow.
                </p>
              )}
            </div>
          ) : null}
        </section>
      ) : null}

      {!disconnected ? (
        <ConnectedCredentialsSection
          projectId={scope.projectId}
          environmentScopeKey={scope.environmentScopeKey}
        />
      ) : null}
      {!disconnected ? (
        <KeyRotationCard
          projectId={scope.projectId}
          environmentScopeKey={scope.environmentScopeKey}
          fingerprintKeyVersion={status.fingerprintKeyVersion}
          pendingKeyVersion={status.pendingKeyVersion}
          onChanged={onConnectionChanged}
        />
      ) : null}
      {status.bootstrapped && !disconnected ? (
        <>
          <ConnectedReportsSection
            projectId={scope.projectId}
            environmentScopeKey={scope.environmentScopeKey}
          />
          <ConnectedNotificationSettingsSection
            projectId={scope.projectId}
            environmentScopeKey={scope.environmentScopeKey}
          />
          <ConnectedAlertWebhooksSection
            projectId={scope.projectId}
            environmentScopeKey={scope.environmentScopeKey}
          />
        </>
      ) : null}

      <section className="settings-delete-card bg-muted">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title settings-card-title-critical">Unlink This Desktop</h2>
        </div>
        <p className="body-muted">
          Removes this site's local binding and fingerprint key. It does not delete the remote site
          or affect other connected environments.
        </p>
        <Button variant="destructive" onClick={() => void handleUnlink()} disabled={unlinking}>
          {unlinking ? "Unlinking..." : "Unlink Site"}
        </Button>
      </section>

      {disconnected ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Resume Watching This Site</h2>
          </div>
          <p className="body-muted">
            The service stopped watching this site, and its state is kept for 30 days from that
            moment. Resuming picks up where it left off: history, lifecycle, and code identity
            intact. CI tokens stay revoked - mint replacements above - and if the site held a deploy
            webhook secret, a new one is shown once when you resume.
          </p>
          <Button onClick={() => void handleReconnect()} disabled={reconnecting}>
            {reconnecting ? "Resuming..." : "Resume Watching"}
          </Button>
        </section>
      ) : (
        <section className="settings-delete-card bg-muted">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title settings-card-title-critical">
              Stop Watching This Site
            </h2>
          </div>
          <p className="body-muted">
            Tells the service to stop watching the site everywhere, not just on this desktop. Its
            data is kept for 30 days in case you resume, and its plan slot frees after a short
            cooldown. This desktop keeps the site binding and fingerprint key so resuming stays
            possible; unlink above if you also want this desktop clean.
          </p>
          <Button
            variant="destructive"
            onClick={() => void handleDisconnect()}
            disabled={disconnecting}>
            {disconnecting ? "Stopping..." : "Stop Watching"}
          </Button>
        </section>
      )}

      {remintedSecret ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Reminted Webhook Secret</h2>
          </div>
          <div className="connected-payload-wrap">
            <div className="row-between">
              <p className="text-13-medium">
                Generation {remintedSecret.secretGeneration} (copy this now)
              </p>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void copyToClipboard(remintedSecret.secret)}>
                Copy Secret
              </Button>
            </div>
            <pre className="connected-payload">{remintedSecret.secret}</pre>
            <p className="text-body-muted">
              Stopping the site revoked every value shown before, so this replaces the secret in
              your deploy pipeline. SiteCMD stores only a fingerprint; this is the one time it is
              readable.
            </p>
          </div>
        </section>
      ) : null}

      <section className="settings-delete-card bg-muted">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title settings-card-title-critical">Erase Site Data</h2>
        </div>
        <p className="body-muted">
          Permanently deletes everything the service holds about this site, immediately. Not
          reversible and not the same as stopping: nothing is retained for a reconnect.
        </p>
        <Button variant="destructive" onClick={() => void handleErase()} disabled={erasing}>
          {erasing ? "Erasing..." : "Erase Site Data"}
        </Button>
      </section>
    </>
  );
}
