import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import {
  acknowledgeAccountRecovery,
  cancelAccountRecovery,
  getAccountRecovery,
  requestAccountRecovery,
} from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import { userFacingError } from "@/lib/user-facing-error";

/** Show and acknowledge subscription-owner recovery state. */
export function AccountRecoveryCard() {
  const toast = useToast();
  const queryClient = useQueryClient();
  const queryKey = queryKeys.settings.accountRecovery();
  const recoveryQuery = useQuery({
    queryKey,
    queryFn: () => getAccountRecovery(),
  });
  const [working, setWorking] = useState(false);
  // One automatic ack per pending recovery per app session: replays are
  // harmless 200 no-ops server-side, but firing on every refetch is noise.
  const acked = useRef<Set<string>>(new Set());

  const pending =
    recoveryQuery.data?.recovery?.status === "pending" ? recoveryQuery.data.recovery : null;

  useEffect(() => {
    if (!pending || acked.current.has(pending.id)) return;
    acked.current.add(pending.id);
    void acknowledgeAccountRecovery().catch(() => {
      // A failed ack must be retryable: the banner stays, the next mount
      // or pending-state change tries again.
      acked.current.delete(pending.id);
    });
  }, [pending]);

  const refresh = () => queryClient.invalidateQueries({ queryKey });

  const handleRequest = async () => {
    setWorking(true);
    try {
      const requested = await requestAccountRecovery();
      await refresh();
      toast.success(
        "Admin recovery requested",
        `Every verified destination is being warned. Unless an admin cancels, it can complete after ${requested.eligibleAt.slice(0, 10)}.`,
      );
    } catch (error) {
      toast.error("Could not request recovery", userFacingError(error, "Try again in a moment."));
    } finally {
      setWorking(false);
    }
  };

  const handleCancel = async () => {
    setWorking(true);
    try {
      await cancelAccountRecovery();
      await refresh();
      toast.success("Recovery cancelled", "The pending request is dead.");
    } catch (error) {
      toast.error(
        "Could not cancel the recovery",
        userFacingError(error, "Try again in a moment."),
      );
    } finally {
      setWorking(false);
    }
  };

  if (pending) {
    return (
      <section className="card card--spacious" role="alert">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title settings-card-title-critical">
            Account Recovery Pending
          </h2>
        </div>
        <p className="body-muted">
          Installation {pending.requestedBy} requested admin recovery on{" "}
          {pending.requestedAt.slice(0, 10)}. Unless an admin cancels it, that machine can become an
          account admin after {pending.eligibleAt.slice(0, 10)}. If nobody on your team did this,
          treat it as an attempted takeover and cancel it now.
        </p>
        <Button variant="destructive" onClick={() => void handleCancel()} disabled={working}>
          {working ? "Cancelling..." : "Cancel Recovery"}
        </Button>
      </section>
    );
  }

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Admin Recovery</h2>
      </div>
      <p className="body-muted">
        If every admin machine is gone, any installation of the subscription can request admin
        recovery. Every verified alert destination is warned immediately, the pending state shows on
        every machine, and an admin can cancel at any time - completion waits 72 hours after the
        warning demonstrably reached you, or 14 days if it never could.
      </p>
      <div className="connected-actions">
        <Button variant="outline" onClick={() => void handleRequest()} disabled={working}>
          {working ? "Requesting..." : "Request Admin Recovery"}
        </Button>
      </div>
    </section>
  );
}
