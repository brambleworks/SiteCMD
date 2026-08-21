import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { ConnectedDestination } from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useToast } from "@/hooks/useToast";
import {
  createConnectedDestination,
  deleteConnectedDestination,
  listConnectedDestinations,
  resendConnectedDestinationVerification,
  updateConnectedDestinationPolicy,
} from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";

/** Refused deletion with the sites that must be detached first. */
interface DeliveryConflict {
  destinationId: string;
  sites: string[];
  message: string;
}

/** Summarize whether mail can reach the destination and why not. */
function deliveryState(destination: ConnectedDestination): {
  tone: string;
  headline: string;
  detail: string;
} {
  if (destination.verification !== "verified") {
    return {
      detail:
        "Nothing is sent here until someone opens the link in the confirmation email. The link lasts 24 hours.",
      headline: "Waiting for confirmation",
      tone: "status-dot-warning",
    };
  }
  if (destination.suppressed) {
    const cause =
      destination.suppressionReason === "complaint"
        ? "This address reported SiteCMD mail as spam"
        : "Mail to this address bounced";
    return {
      detail: `${cause}, so sending stopped. Confirming again is the way back: send the email and open its link.`,
      headline: "Suppressed",
      tone: "status-dot-critical",
    };
  }
  const disabled: string[] = [];
  if (destination.immediateDisabled) disabled.push("immediate alerts are off");
  if (destination.digestDisabled) disabled.push("the digest is off");
  return {
    detail:
      disabled.length > 0
        ? `Confirmed, and ${disabled.join(" and ")}. Unsubscribe links in the emails turn these off; this is where they come back on.`
        : "Confirmed. Immediate alerts and the digest both reach this address.",
    headline: "Confirmed",
    tone: disabled.length > 0 ? "status-dot-info status-dot-dim" : "status-dot-success",
  };
}

/** Manages immutable, account-level alert addresses and their consent state. */
export function ConnectedAlertDestinationsSection() {
  const toast = useToast();
  const queryClient = useQueryClient();
  const queryKey = queryKeys.settings.connectedDestinations();
  const destinationsQuery = useQuery({
    queryKey,
    queryFn: () => listConnectedDestinations(),
  });
  const [newAddress, setNewAddress] = useState("");
  const [adding, setAdding] = useState(false);
  const [busyDestination, setBusyDestination] = useState<string | null>(null);
  const [conflict, setConflict] = useState<DeliveryConflict | null>(null);
  const [resendNotice, setResendNotice] = useState<string | null>(null);

  const refresh = () => queryClient.invalidateQueries({ queryKey });

  const handleAdd = async () => {
    setAdding(true);
    try {
      const created = await createConnectedDestination({ address: newAddress.trim() });
      setNewAddress("");
      await refresh();
      toast.success(
        created.verification === "verified"
          ? "That address is already on this account"
          : "Confirmation email on its way",
        created.verification === "verified"
          ? "It is already confirmed, so alerts can be pointed at it now."
          : "Nothing reaches it until someone opens the link in that email.",
      );
    } catch (error) {
      toast.error("Could not add that address", String(error));
    } finally {
      setAdding(false);
    }
  };

  const handleResend = async (destinationId: string) => {
    setBusyDestination(destinationId);
    setResendNotice(null);
    try {
      const outcome = await resendConnectedDestinationVerification({ destinationId });
      if (!outcome.sent) {
        setResendNotice(outcome.message);
        return;
      }
      await refresh();
      toast.success("Confirmation email sent again", "The new link lasts 24 hours.");
    } catch (error) {
      toast.error("Could not send the confirmation email", String(error));
    } finally {
      setBusyDestination(null);
    }
  };

  const handlePolicy = async (
    destination: ConnectedDestination,
    next: { immediateDisabled: boolean; digestDisabled: boolean },
  ) => {
    setBusyDestination(destination.destinationId);
    try {
      const outcome = await updateConnectedDestinationPolicy({
        ...next,
        destinationId: destination.destinationId,
        revision: destination.revision,
      });
      await refresh();
      if (!outcome.applied) toast.error("Nothing changed", outcome.message);
    } catch (error) {
      toast.error("Could not change what this address receives", String(error));
    } finally {
      setBusyDestination(null);
    }
  };

  const handleDelete = async (destinationId: string) => {
    setBusyDestination(destinationId);
    setConflict(null);
    try {
      const outcome = await deleteConnectedDestination({ destinationId });
      if (!outcome.deleted) {
        setConflict({ destinationId, message: outcome.message, sites: outcome.sites });
        return;
      }
      await refresh();
      toast.success("Address removed");
    } catch (error) {
      toast.error("Could not remove that address", String(error));
    } finally {
      setBusyDestination(null);
    }
  };

  const destinations = destinationsQuery.data ?? [];

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Alert Email Addresses</h2>
      </div>
      <p className="body-muted">
        The human channel, shared by every site on this account. SiteCMD emails an address once to
        ask whether it wants alerts, and delivers nothing until that mailbox opens the link. An
        address that bounces or reports spam stops receiving and says so here. Addresses cannot be
        edited: to move a site's alerts, add the new address and point the site at it in that site's
        alert settings.
      </p>
      {destinationsQuery.isError ? (
        <p className="agent-handoff-error" role="alert">
          Alert addresses could not load.
        </p>
      ) : null}
      {destinations.length > 0 ? (
        <div className="webhook-list">
          {destinations.map((destination) => {
            const state = deliveryState(destination);
            const busy = busyDestination === destination.destinationId;
            const unusable = destination.verification !== "verified" || destination.suppressed;
            return (
              <div key={destination.destinationId} className="settings-webhook-row">
                <span className={state.tone} />
                <div className="flex-fill min-w-0">
                  <div className="text-mono-sm text-truncate">
                    {destination.address ?? destination.destinationId}
                  </div>
                  <div className="text-13-muted">
                    {state.headline}. {state.detail}
                  </div>
                </div>
                {unusable ? (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => void handleResend(destination.destinationId)}
                    disabled={busy}>
                    Send Confirmation Again
                  </Button>
                ) : (
                  <>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() =>
                        void handlePolicy(destination, {
                          digestDisabled: destination.digestDisabled,
                          immediateDisabled: !destination.immediateDisabled,
                        })
                      }
                      disabled={busy}>
                      {destination.immediateDisabled ? "Resume Alerts" : "Pause Alerts"}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() =>
                        void handlePolicy(destination, {
                          digestDisabled: !destination.digestDisabled,
                          immediateDisabled: destination.immediateDisabled,
                        })
                      }
                      disabled={busy}>
                      {destination.digestDisabled ? "Resume Digest" : "Pause Digest"}
                    </Button>
                  </>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleDelete(destination.destinationId)}
                  disabled={busy}>
                  Remove
                </Button>
              </div>
            );
          })}
        </div>
      ) : destinationsQuery.isSuccess ? (
        <p className="body-muted">No alert addresses yet. Add the first one below.</p>
      ) : null}
      {resendNotice ? (
        <p className="agent-handoff-error" role="status">
          {resendNotice}
        </p>
      ) : null}
      {conflict ? (
        <div className="connected-payload-wrap">
          <p className="text-13-medium" role="status">
            {conflict.message}
          </p>
          <p className="text-body-muted">
            {conflict.sites.length > 0
              ? `Still delivering here: ${conflict.sites.join(", ")}. Open each site's alert settings and choose a different address, then remove this one.`
              : "Open the alert settings of the sites using this address and choose a different one, then remove this one."}
          </p>
          <Button size="sm" variant="ghost" onClick={() => setConflict(null)}>
            Dismiss
          </Button>
        </div>
      ) : null}
      <div className="stack-base connected-form">
        <label className="form-label" htmlFor="connected-destination-address">
          Email address
        </label>
        <Input
          id="connected-destination-address"
          type="email"
          autoComplete="off"
          placeholder="alerts@example.com"
          value={newAddress}
          onChange={(event) => setNewAddress(event.target.value)}
        />
        <Button onClick={() => void handleAdd()} disabled={adding || !newAddress.trim()}>
          {adding ? "Adding..." : "Add Address"}
        </Button>
        <p className="text-body-muted">
          Adding an address that is already here changes nothing and sends no second email. Use Send
          Confirmation Again for that.
        </p>
      </div>
    </section>
  );
}
