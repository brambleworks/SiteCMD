import type { RefObject } from "react";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  resolvePendingVerification,
  type PendingVerificationEntry,
} from "@/lib/pending-verification";
import { formatRelativeTime } from "@/lib/format";
import { useCurrentTime } from "@/lib/useCurrentTime";

export function PendingSearchVerificationSection({
  pendingEntries,
  sectionRef,
  verifyingAllPending,
  verifyingCheckId,
  verifyingPendingId,
  onVerifyAll,
  onVerifyEntry,
}: {
  pendingEntries: PendingVerificationEntry[];
  sectionRef: RefObject<HTMLDivElement | null>;
  verifyingAllPending: boolean;
  verifyingCheckId: string | null;
  verifyingPendingId: string | null;
  onVerifyAll: () => void;
  onVerifyEntry: (entry: PendingVerificationEntry) => void;
}) {
  const nowMs = useCurrentTime();

  if (pendingEntries.length === 0) return null;
  const verificationDisabled =
    verifyingAllPending || verifyingPendingId !== null || verifyingCheckId !== null;

  return (
    <div ref={sectionRef} className="card card--spacious">
      <div className="row-between-top">
        <div className="flex-fill">
          <span className="section-label-lg">Recent SEO Changes</span>
          <p className="text-sm-bold search-changes-lede">
            {pendingEntries.length === 1
              ? "1 SEO follow-up can be re-checked"
              : `${pendingEntries.length} SEO follow-ups can be re-checked`}
          </p>
          <p className="body-desc">
            You changed a search-related file or used a fix action here. Re-check the exact SEO
            cluster when you want a fresh result.
          </p>
        </div>
        <div className="row-actions">
          <Button
            variant="ghost"
            className="verify-action-btn btn--gap-tight"
            disabled={verificationDisabled}
            onClick={onVerifyAll}>
            {verifyingAllPending ? <Loader2 className="spinner-sm" /> : null}
            Verify all
          </Button>
          <Button
            variant="ghost"
            className="verify-action-btn"
            onClick={() => {
              for (const entry of pendingEntries) {
                resolvePendingVerification(entry.id);
              }
            }}>
            Clear
          </Button>
        </div>
      </div>

      <div className="search-changes-list">
        {pendingEntries.slice(0, 4).map((entry) => (
          <div key={entry.id} className="row-card">
            <div className="flex-fill stack-tight">
              <p className="row-title">{entry.label}</p>
              <p className="subtitle-xs text-truncate">
                {entry.reason} &middot; queued {formatRelativeTime(entry.updatedAt, nowMs)}
              </p>
            </div>
            <div className="row-actions">
              <Button
                variant="ghost"
                className="verify-action-btn btn--gap-tight"
                disabled={verificationDisabled}
                onClick={() => onVerifyEntry(entry)}>
                {verifyingPendingId === entry.id ? <Loader2 className="spinner-sm" /> : null}
                Verify now
              </Button>
              <Button
                variant="ghost"
                className="verify-action-btn muted-text"
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
