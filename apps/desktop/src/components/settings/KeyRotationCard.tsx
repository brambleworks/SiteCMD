import { useState } from "react";
import type { ConnectedKeyRotation } from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { abortConnectedKeyRotation, rotateConnectedFingerprintKey } from "@/lib/commands";
import { userFacingError } from "@/lib/user-facing-error";

interface KeyRotationCardProps {
  projectId: number;
  environmentScopeKey: string;
  fingerprintKeyVersion: number;
  pendingKeyVersion: number | null;
  /** Called after a claim or abort so the parent re-reads status. */
  onChanged: () => Promise<void>;
}

/** Manages the site's code-identity key epoch without exposing key material. */
export function KeyRotationCard({
  projectId,
  environmentScopeKey,
  fingerprintKeyVersion,
  pendingKeyVersion,
  onChanged,
}: KeyRotationCardProps) {
  const toast = useToast();
  const scope = { environmentScopeKey, projectId };
  const [working, setWorking] = useState(false);
  // A claim held elsewhere, learned from an already_pending answer. Not in
  // local status because this desktop holds no candidate for it.
  const [elsewhere, setElsewhere] = useState<ConnectedKeyRotation | null>(null);

  const handleRotate = async () => {
    setWorking(true);
    try {
      const claim = await rotateConnectedFingerprintKey(scope);
      if (claim.status === "claimed") {
        setElsewhere(null);
        await onChanged();
        toast.success(
          `Rotation to version ${claim.version} claimed`,
          "Run a code scan covering the whole project, then Sync Now to complete it.",
        );
      } else {
        setElsewhere(claim);
      }
    } catch (error) {
      toast.error(
        "Could not start the key rotation",
        userFacingError(error, "Try again in a moment."),
      );
    } finally {
      setWorking(false);
    }
  };

  const handleAbort = async () => {
    setWorking(true);
    try {
      await abortConnectedKeyRotation(scope);
      setElsewhere(null);
      await onChanged();
      toast.success("Key rotation aborted", "The claimed version number stays burned.");
    } catch (error) {
      toast.error("Could not abort the rotation", userFacingError(error, "Try again in a moment."));
    } finally {
      setWorking(false);
    }
  };

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Code Identity Key</h2>
      </div>
      <p className="body-muted">
        Code findings are matched across machines under a keyed fingerprint. This desktop holds
        version {fingerprintKeyVersion}. Rotate it if a CI secret leaked or a machine holding the
        key is gone: the new key is minted here, the service learns only its commitment, and the
        switch happens when a complete code scan syncs under the new version.
      </p>
      {pendingKeyVersion !== null ? (
        <div className="stack-base connected-form">
          <p className="text-13-medium">
            Rotation to version {pendingKeyVersion} is pending on this desktop.
          </p>
          <p className="text-body-muted">
            To complete it, run a code scan covering the whole project, then Sync Now. Until then
            version {fingerprintKeyVersion} stays in force. After completion, mint a new CI token
            key for your pipeline - the old key stops being accepted.
          </p>
          <Button variant="outline" onClick={() => void handleAbort()} disabled={working}>
            {working ? "Working..." : "Abort Rotation"}
          </Button>
        </div>
      ) : (
        <div className="connected-actions">
          <Button variant="outline" onClick={() => void handleRotate()} disabled={working}>
            {working ? "Claiming..." : "Rotate Key"}
          </Button>
        </div>
      )}
      {elsewhere ? (
        <div className="stack-base connected-form">
          <p className="text-13-medium">
            A rotation to version {elsewhere.version} is already pending
            {elsewhere.claimedBy
              ? ` on installation ${elsewhere.claimedBy}`
              : " on another machine"}
            {elsewhere.expiresAt ? `, until ${elsewhere.expiresAt.slice(0, 10)}` : ""}.
          </p>
          <p className="text-body-muted">
            If that machine is gone and will never finish, abort its claim and rotate again from
            here.
          </p>
          <Button variant="outline" onClick={() => void handleAbort()} disabled={working}>
            {working ? "Working..." : "Abort That Claim"}
          </Button>
        </div>
      ) : null}
    </section>
  );
}
