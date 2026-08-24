import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "@/hooks/useToast";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import {
  cancelFixAttempt,
  getFixAttemptForIssue,
  isAttemptActive,
  type FixAttempt,
} from "@/lib/fix-attempts";
import { userFacingError } from "@/lib/user-facing-error";

// Safety poll for events missed while the listener was attaching.
const ACTIVE_POLL_INTERVAL_MS = 5_000;

/** Skip state updates when a safety poll returns an unchanged attempt. */
function sameAttempt(a: FixAttempt | null, b: FixAttempt | null): boolean {
  if (a == null || b == null) return a === b;
  return (
    a.id === b.id &&
    a.status === b.status &&
    a.updatedAt === b.updatedAt &&
    a.failureDetail === b.failureDetail &&
    a.agentSummary === b.agentSummary
  );
}

/** Manages one issue's current fix attempt, refresh, cancellation, and toast state. */
export function useFixAttempt({
  projectId,
  envUrl,
  checkId,
  title,
}: {
  projectId: number | null | undefined;
  envUrl: string | null | undefined;
  checkId: string;
  title: string;
}): {
  attempt: FixAttempt | null;
  setAttempt: (a: FixAttempt | null) => void;
  cancel: () => Promise<void>;
} {
  const { success, error } = useToast();
  const [attempt, setAttemptState] = useState<FixAttempt | null>(null);
  // Mirrors the latest applied attempt so refetch can drop no-op poll results
  // without depending on `attempt` (which would churn the listener effects).
  const attemptRef = useRef<FixAttempt | null>(null);
  // Monotonic counter: each refetch (and each external setAttempt) bumps it so
  // an out-of-order response from a slower earlier fetch is discarded.
  const fetchSeqRef = useRef(0);
  const disposedRef = useRef(false);
  // Attempt-and-status key prevents duplicate verified notifications.
  const prevVerifyKeyRef = useRef<string | null>(null);

  useEffect(() => {
    disposedRef.current = false;
    return () => {
      disposedRef.current = true;
    };
  }, []);

  const applyAttempt = useCallback((next: FixAttempt | null) => {
    attemptRef.current = next;
    setAttemptState(next);
  }, []);

  const refetch = useCallback(async () => {
    if (projectId == null || !envUrl) return;
    const seq = ++fetchSeqRef.current;
    let next: FixAttempt | null;
    try {
      next = await getFixAttemptForIssue(projectId, envUrl, checkId, title);
    } catch {
      // Keep the last known attempt on a transient fetch failure; the event
      // listener or poll will retry.
      return;
    }
    if (disposedRef.current || seq !== fetchSeqRef.current) return;
    // The safety poll mostly returns an unchanged row; skip the no-op update.
    if (sameAttempt(attemptRef.current, next)) return;
    applyAttempt(next);
  }, [projectId, envUrl, checkId, title, applyAttempt]);

  // Prevent an in-flight refetch from overwriting a newly created attempt.
  const setAttempt = useCallback(
    (next: FixAttempt | null) => {
      fetchSeqRef.current += 1;
      applyAttempt(next);
    },
    [applyAttempt],
  );

  // Load on mount and whenever the issue identity changes. Reset to null first
  // so the previous issue's attempt never bleeds into the new dossier.
  useEffect(() => {
    fetchSeqRef.current += 1;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- clears the prior attempt and refetches when the target changes; bumps the fetch-sequence guard
    applyAttempt(null);
    void refetch();
  }, [refetch, applyAttempt]);

  // Live refresh: the Rust watcher emits "fix-attempt-updated" on every change.
  useTauriEvent("fix-attempt-updated", () => {
    void refetch();
  });

  // Safety poll while the attempt is active; stops once it reaches a terminal
  // status (verified / verify_failed / canceled / expired).
  const pollWhileActive = attempt != null && isAttemptActive(attempt.status);
  useEffect(() => {
    if (!pollWhileActive) return;
    const id = window.setInterval(() => {
      void refetch();
    }, ACTIVE_POLL_INTERVAL_MS);
    return () => window.clearInterval(id);
  }, [pollWhileActive, refetch]);

  // Notify once on a verified transition, but stay silent on initial load.
  useEffect(() => {
    const key = attempt ? `${attempt.id}:${attempt.status}` : null;
    const prev = prevVerifyKeyRef.current;
    prevVerifyKeyRef.current = key;
    if (attempt?.status !== "verified" || prev == null || prev === key) return;
    success("Fix verified", "SiteCMD re-ran the check and it passes now.");
  }, [attempt, success]);

  const cancel = useCallback(async () => {
    if (attempt == null) return;
    try {
      await cancelFixAttempt(attempt.id);
    } catch (err) {
      error("Could not cancel the fix attempt", userFacingError(err, "Try again in a moment."));
    }
    // Refetch even after a failed cancel: the attempt may have changed
    // server-side (e.g. it was already canceled or expired).
    await refetch();
  }, [attempt, error, refetch]);

  return { attempt, setAttempt, cancel };
}
