import { useEffect, useState, type ReactNode } from "react";
import { Ban, CheckCircle2, Loader2, RotateCcw, SkipForward } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { blockIssue, getIssueState, ignoreIssue, reopenIssue } from "@/lib/issues";
import { ISSUE_TRIAGE_COPY, TRIAGE_SCORE_RECOVERY_NOTE } from "@/lib/issue-triage-copy";
import type { WorkItemStatus } from "@/lib/project-summary-types";
import { cn } from "@/lib/utils";
import { userFacingError } from "@/lib/user-facing-error";

type LifecycleAction = "ignored" | "blocked" | "reopened";

// Reason persisted when an issue is blocked straight from the dossier action bar.
const DOSSIER_BLOCK_REASON = "Marked not relevant from the dossier";

interface IssueActionBarProps {
  projectId?: number | null;
  // Lifecycle persistence requires the full project, environment, and check identity.
  checkId?: string | null;
  envUrl?: string | null;
  // Lifecycle writes use the group's canonical check ID.
  initialStatus?: WorkItemStatus | null;
  verifyAction?: {
    label?: string;
    onClick: () => void | Promise<void>;
    verifying?: boolean;
    disabled?: boolean;
  };
  onIgnore?: () => void | Promise<void>;
  onBlock?: () => void | Promise<void>;
  onReopen?: () => void | Promise<void>;
  extraActions?: ReactNode;
  className?: string;
}

// Includes the derived state for an active fix attempt.
const WORK_ITEM_STATUSES: ReadonlySet<WorkItemStatus> = new Set<WorkItemStatus>([
  "new",
  "working",
  "verified",
  "ignored",
  "blocked",
  "snoozed",
  "regressed",
]);

// Map a project_issue_states status string onto the bar's status vocabulary.
// Unknown persisted values are corruption, not an active issue.
function toWorkItemStatus(raw: string | null | undefined): WorkItemStatus | null {
  if (!raw) return null;
  if (!WORK_ITEM_STATUSES.has(raw as WorkItemStatus)) {
    throw new Error(`Unknown persisted issue status: ${raw}`);
  }
  return raw as WorkItemStatus;
}

export function IssueActionBar({
  projectId,
  checkId,
  envUrl,
  initialStatus = null,
  verifyAction,
  onIgnore,
  onBlock,
  onReopen,
  extraActions,
  className,
}: IssueActionBarProps) {
  const { error } = useToast();
  // Lifecycle persists to project_issue_states, keyed by check_id + env_url.
  const canPersistStatus = projectId != null && Boolean(checkId) && Boolean(envUrl);
  const [status, setStatus] = useState<WorkItemStatus | null>(initialStatus ?? null);
  const [pending, setPending] = useState<LifecycleAction | null>(null);
  const [hydrating, setHydrating] = useState(canPersistStatus && initialStatus == null);
  const [hydrationError, setHydrationError] = useState(false);
  const isPaused = status === "ignored" || status === "blocked";
  const hasLifecycleActions = isPaused || Boolean(onIgnore || onBlock);

  // Score events rehydrate lifecycle state changed by background verification.
  const [statusRefresh, setStatusRefresh] = useState(0);
  useTauriEvent(
    "site-score-changed",
    (payload) => {
      if (projectId != null && payload?.projectId !== projectId) return;
      setStatusRefresh((token) => token + 1);
    },
    { enabled: canPersistStatus },
  );

  useEffect(() => {
    if (!canPersistStatus) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- seeds status from props when persistence is unavailable; the else branch hydrates asynchronously
      setStatus(initialStatus ?? null);
      setHydrating(false);
      setHydrationError(false);
      return;
    }

    // Hydrate the badge from project_issue_states.
    setStatus(initialStatus ?? null);
    setHydrationError(false);
    if (initialStatus != null) {
      setHydrating(false);
      return;
    }
    let cancelled = false;
    setHydrating(true);
    void getIssueState(projectId as number, envUrl as string, checkId as string)
      .then((row) => {
        if (!cancelled) setStatus(row ? toWorkItemStatus(row[0]) : null);
      })
      .catch(() => {
        if (!cancelled) {
          setStatus(null);
          setHydrationError(true);
        }
      })
      .finally(() => {
        if (!cancelled) setHydrating(false);
      });
    return () => {
      cancelled = true;
    };
  }, [canPersistStatus, initialStatus, projectId, checkId, envUrl, statusRefresh]);

  const persistStatus = async (nextStatus: WorkItemStatus) => {
    if (!canPersistStatus) return;
    const pid = projectId as number;
    const url = envUrl as string;
    // One canonical group check_id owns every source/location occurrence.
    const cid = checkId as string;
    if (nextStatus === "ignored") {
      await ignoreIssue(pid, url, cid);
    } else if (nextStatus === "blocked") {
      await blockIssue(pid, url, cid, DOSSIER_BLOCK_REASON);
    } else {
      await reopenIssue(pid, url, cid);
    }
  };

  const run = async (
    action: LifecycleAction,
    nextStatus: WorkItemStatus,
    cb?: () => void | Promise<void>,
  ) => {
    try {
      setPending(action);
      setHydrating(false);
      if (canPersistStatus) {
        await persistStatus(nextStatus);
      }
      await cb?.();
      setStatus(nextStatus);
    } catch (err) {
      error(
        "Could not update issue status",
        userFacingError(err, "Your change was not saved. Try again."),
      );
    } finally {
      setPending(null);
    }
  };

  const verifyButton = verifyAction ? (
    <Button
      variant="success"
      className="issue-action-button"
      onClick={() => void verifyAction.onClick()}
      disabled={verifyAction.disabled || verifyAction.verifying || hydrating || hydrationError}>
      {verifyAction.verifying ? (
        <Loader2 className="spinner-sm" />
      ) : (
        <CheckCircle2 className="icon-sm" />
      )}
      <span>{verifyAction.label ?? "Verify fix"}</span>
    </Button>
  ) : null;

  return (
    <div className={cn("stack-snug", className)}>
      {hydrationError ? (
        <div className="issue-action-note">
          <div className="body-muted">
            Issue status could not load. Retry before changing its lifecycle.
          </div>
          <Button
            size="sm"
            variant="outline"
            onClick={() => setStatusRefresh((value) => value + 1)}>
            Retry
          </Button>
        </div>
      ) : null}

      {verifyButton ? <div className="issue-action-primary">{verifyButton}</div> : null}

      {extraActions ? <div className="issue-action-extra">{extraActions}</div> : null}

      {hasLifecycleActions ? (
        <div className="issue-action-lifecycle">
          {isPaused ? (
            <Button
              variant="outline"
              className="issue-action-button"
              onClick={() => void run("reopened", "new", onReopen)}
              disabled={pending != null || hydrating || hydrationError}>
              {pending === "reopened" ? (
                <Loader2 className="spinner-sm" />
              ) : (
                <RotateCcw className="icon-sm" />
              )}
              <span>Reopen</span>
            </Button>
          ) : (
            <>
              {onIgnore ? (
                <Button
                  variant="outline"
                  className="issue-action-button"
                  title={ISSUE_TRIAGE_COPY.ignore.help}
                  onClick={() => void run("ignored", "ignored", onIgnore)}
                  disabled={pending != null || hydrating || hydrationError}>
                  {pending === "ignored" ? (
                    <Loader2 className="spinner-sm" />
                  ) : (
                    <SkipForward className="icon-sm" />
                  )}
                  <span>{ISSUE_TRIAGE_COPY.ignore.label}</span>
                </Button>
              ) : null}
              {onBlock ? (
                <Button
                  variant="outline"
                  className="issue-action-button"
                  title={ISSUE_TRIAGE_COPY.block.help}
                  onClick={() => void run("blocked", "blocked", onBlock)}
                  disabled={pending != null || hydrating || hydrationError}>
                  {pending === "blocked" ? (
                    <Loader2 className="spinner-sm" />
                  ) : (
                    <Ban className="icon-sm" />
                  )}
                  <span>{ISSUE_TRIAGE_COPY.block.label}</span>
                </Button>
              ) : null}
            </>
          )}
        </div>
      ) : null}

      {!isPaused && (onIgnore || onBlock) ? (
        <p className="issue-action-help body-muted">{TRIAGE_SCORE_RECOVERY_NOTE}</p>
      ) : null}
    </div>
  );
}
