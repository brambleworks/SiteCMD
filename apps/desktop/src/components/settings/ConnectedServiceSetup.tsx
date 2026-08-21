import { useState } from "react";
import type { ConnectedSiteChallenge, ConnectedStatus } from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useToast } from "@/hooks/useToast";
import {
  activateConnectedService,
  createConnectedSite,
  importConnectedConnection,
  type ConnectedScopeArgs,
} from "@/lib/commands";

interface ConnectedServiceSetupProps {
  scope: ConnectedScopeArgs;
  status: ConnectedStatus;
  onChallenge: (challenge: ConnectedSiteChallenge) => void;
  onStatusChanged: () => Promise<void>;
}

export function ConnectedServiceSetup({
  scope,
  status,
  onChallenge,
  onStatusChanged,
}: ConnectedServiceSetupProps) {
  const toast = useToast();
  const [siteUrl, setSiteUrl] = useState("");
  const [newSiteToken, setNewSiteToken] = useState("");
  const [creating, setCreating] = useState(false);
  const [activatingService, setActivatingService] = useState(false);
  const [encryptedImport, setEncryptedImport] = useState("");
  const [importPassphrase, setImportPassphrase] = useState("");
  const [installationToken, setInstallationToken] = useState("");
  const [importing, setImporting] = useState(false);

  const handleActivateService = async () => {
    setActivatingService(true);
    try {
      const activation = await activateConnectedService();
      await onStatusChanged();
      toast.success(
        "Connected service activated",
        `Your ${activation.tier === "pro" ? "Pro" : "Plus"} subscription now covers this desktop. No token needed below.`,
      );
    } catch (error) {
      toast.error("Could not activate the connected service", String(error));
    } finally {
      setActivatingService(false);
    }
  };

  const handleCreate = async () => {
    setCreating(true);
    try {
      const created = await createConnectedSite({
        ...scope,
        url: siteUrl.trim(),
        installationToken: newSiteToken.trim(),
      });
      onChallenge(created);
      setSiteUrl("");
      setNewSiteToken("");
      await onStatusChanged();
      toast.success(
        "Site created",
        "Publish the challenge below, then verify. Nothing is scanned until you do.",
      );
    } catch (error) {
      toast.error("Could not create the site", String(error));
    } finally {
      setCreating(false);
    }
  };

  const handleImport = async () => {
    setImporting(true);
    try {
      await importConnectedConnection({
        ...scope,
        encryptedExport: encryptedImport.trim(),
        passphrase: importPassphrase,
        installationToken: installationToken.trim(),
      });
      setEncryptedImport("");
      setImportPassphrase("");
      setInstallationToken("");
      await onStatusChanged();
      toast.success("Connection imported", "This desktop can now inspect and sync the site.");
    } catch (error) {
      toast.error("Connection import failed", String(error));
    } finally {
      setImporting(false);
    }
  };

  return (
    <>
      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Connect This Environment</h2>
        </div>
        <p className="body-muted">
          Creates a new connected site for this production URL. You will be asked to prove you own
          the domain before anything is scanned.
        </p>
        {status.hasInstallationToken ? (
          <p className="body-muted">
            This desktop already holds its connected-service credential, so no token is needed
            below.
          </p>
        ) : (
          <div className="stack-base connected-form">
            <p className="body-muted">
              Your license includes the connected service. Activate it once on this desktop and
              every step below works without a pasted token.
            </p>
            <Button
              variant="outline"
              onClick={() => void handleActivateService()}
              disabled={activatingService || !status.endpointConfigured}>
              {activatingService ? "Activating..." : "Activate with Your License"}
            </Button>
          </div>
        )}
        <div className="stack-base connected-form">
          <label className="form-label" htmlFor="connected-new-url">
            Production URL
          </label>
          <Input
            id="connected-new-url"
            type="url"
            autoComplete="off"
            placeholder="https://example.com"
            value={siteUrl}
            onChange={(event) => setSiteUrl(event.target.value)}
          />
          <label className="form-label" htmlFor="connected-new-token">
            Installation token (only when moving one by hand)
          </label>
          <Input
            id="connected-new-token"
            type="password"
            autoComplete="off"
            value={newSiteToken}
            onChange={(event) => setNewSiteToken(event.target.value)}
          />
          <Button
            onClick={() => void handleCreate()}
            disabled={
              creating ||
              !status.endpointConfigured ||
              !siteUrl.trim() ||
              (!status.hasInstallationToken && !newSiteToken.trim())
            }>
            {creating ? "Creating..." : "Create Connected Site"}
          </Button>
        </div>
      </section>

      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Import Site Connection</h2>
        </div>
        <p className="body-muted">
          Pair the encrypted connection from the site-owning desktop with this installation's own
          token. The export authorizes nothing by itself.
        </p>
        <div className="stack-base connected-form">
          <label className="form-label" htmlFor="connected-import-payload">
            Encrypted connection export
          </label>
          <textarea
            id="connected-import-payload"
            className="field-control field-control--muted connected-secret-textarea"
            value={encryptedImport}
            onChange={(event) => setEncryptedImport(event.target.value)}
            spellCheck={false}
          />
          <label className="form-label" htmlFor="connected-import-passphrase">
            Export passphrase
          </label>
          <Input
            id="connected-import-passphrase"
            type="password"
            autoComplete="off"
            value={importPassphrase}
            onChange={(event) => setImportPassphrase(event.target.value)}
          />
          <label className="form-label" htmlFor="connected-installation-token">
            This installation's token (only when moving one by hand)
          </label>
          <Input
            id="connected-installation-token"
            type="password"
            autoComplete="off"
            value={installationToken}
            onChange={(event) => setInstallationToken(event.target.value)}
          />
          <Button
            onClick={() => void handleImport()}
            disabled={
              importing ||
              !encryptedImport.trim() ||
              !importPassphrase ||
              (!status.hasInstallationToken && !installationToken.trim())
            }>
            {importing ? "Importing..." : "Import Connection"}
          </Button>
        </div>
      </section>
    </>
  );
}
