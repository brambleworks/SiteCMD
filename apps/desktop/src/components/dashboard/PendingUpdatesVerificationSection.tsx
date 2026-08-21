import type { RefObject } from "react";
import { ListChecks, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  resolvePendingVerification,
  type PendingVerificationEntry,
} from "@/lib/pending-verification";
import { formatRelativeTime } from "@/lib/format";
import { useCurrentTime } from "@/lib/useCurrentTime";

export function PendingUpdatesVerificationSection({
  sectionRef,
  pendingEntries,
  verifyingAllPending,
  verifyingPendingId,
  verifyingUpdateKey,
  onVerifyAll,
  onVerifyEntry,
}: {
  sectionRef: RefObject<HTMLDivElement | null>;
  pendingEntries: PendingVerificationEntry[];
  verifyingAllPending: boolean;
  verifyingPendingId: string | null;
  verifyingUpdateKey: string | null;
  onVerifyAll: () => void;
  onVerifyEntry: (entry: PendingVerificationEntry) => void;
}) {
  const nowMs = useCurrentTime();

  if (pendingEntries.length === 0) return null;
  const verificationDisabled =
    verifyingAllPending || verifyingPendingId !== null || verifyingUpdateKey !== null;

  return (
    <div ref={sectionRef} className="card card--spacious">
      <div className="row-between-top">
        <div className="flex-fill">
          <span className="card__title">
            <ListChecks className="card__icon icon-md" aria-hidden="true" />
            <span>Recent Dependency Changes</span>
          </span>
          <p className="text-sm-bold pending-updates-lede">
            {pendingEntries.length === 1
              ? "1 dependency follow-up can be re-checked"
              : `${pendingEntries.length} dependency follow-ups can be re-checked`}
          </p>
          <p className="body-desc">
            You changed dependency-related files or used a fix action here. Run the audit again when
            you want SiteCMD to confirm what changed.
          </p>
        </div>
        <div className="pending-updates-actions">
          <Button
            variant="outline"
            size="sm"
            onClick={onVerifyAll}
            disabled={verificationDisabled}
            className="btn--gap-tight">
            {verifyingAllPending ? <Loader2 className="icon-sm animate-spin" /> : null}
            Verify all
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              for (const entry of pendingEntries) {
                resolvePendingVerification(entry.id);
              }
            }}>
            Clear
          </Button>
        </div>
      </div>

      <div className="pending-updates-list">
        {pendingEntries.slice(0, 4).map((entry) => (
          <div key={entry.id} className="row-card">
            <div className="pending-updates-row-copy">
              <p className="row-title">{entry.label}</p>
              <p className="subtitle-xs text-truncate">
                {entry.reason} &middot; queued {formatRelativeTime(entry.updatedAt, nowMs)}
              </p>
            </div>
            <div className="pending-updates-actions">
              <Button
                variant="outline"
                size="sm"
                onClick={() => onVerifyEntry(entry)}
                disabled={verificationDisabled}
                className="btn--gap-tight">
                {verifyingPendingId === entry.id ? (
                  <Loader2 className="icon-sm animate-spin" />
                ) : null}
                Verify now
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => resolvePendingVerification(entry.id)}>
                Dismiss
              </Button>
            </div>
          </div>
        ))}
        {pendingEntries.length > 4 ? (
          <p className="subtitle-xs">{pendingEntries.length - 4} more available to re-check.</p>
        ) : null}
      </div>
    </div>
  );
}
