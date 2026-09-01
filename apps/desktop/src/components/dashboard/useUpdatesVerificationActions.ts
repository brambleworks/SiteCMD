import { useCallback, useState, type MutableRefObject } from "react";

import { addJob, completeJob, failJob } from "@/lib/jobs";
import type { PendingVerificationEntry } from "@/lib/pending-verification";
import { resolvePendingVerification } from "@/lib/pending-verification";
import { markUpdateStillPending, markUpdateVerified } from "@/lib/update-memory";
import { buildUpdateCampaignCopy, formatPackageUpdateSummary } from "@/lib/update-priority";
import { buildUpdateQueueSummary } from "@/lib/update-summary";
import type { PackageUpdate, UpdateReport } from "@/lib/types";

import { getClearedUpdates } from "./update-history";
import { useUpdateFollowUps } from "./useUpdateFollowUps";
import { useUpdateTimelineRecorder } from "./useUpdateTimelineRecorder";
import { findPackageUpdateByItemId } from "./updates-page-model";
import { userFacingError } from "@/lib/user-facing-error";

const VERIFICATION_FALLBACK = "Run the verification again after the site has deployed.";

interface UpdatesToast {
  success: (title: string, message?: string) => void;
  warning: (title: string, message?: string) => void;
  error: (title: string, message?: string) => void;
}

interface LoadReportOptions {
  showToast?: boolean;
}

interface UseUpdatesVerificationActionsOptions {
  hostname: string;
  loadReport: (options?: LoadReportOptions) => Promise<UpdateReport | null>;
  loadUpdateHistory: () => Promise<void>;
  normalizedUrl: string;
  pendingUpdateEntries: PendingVerificationEntry[];
  projectId: number;
  projectName: string;
  projectPath: string | null;
  report: UpdateReport | null;
  reportRef: MutableRefObject<UpdateReport | null>;
  toast: UpdatesToast;
}

export function useUpdatesVerificationActions({
  hostname,
  loadReport,
  loadUpdateHistory,
  normalizedUrl,
  pendingUpdateEntries,
  projectId,
  projectName,
  projectPath,
  report,
  reportRef,
  toast,
}: UseUpdatesVerificationActionsOptions) {
  const [verifyingUpdateKey, setVerifyingUpdateKey] = useState<string | null>(null);
  const [verifyingPendingId, setVerifyingPendingId] = useState<string | null>(null);
  const [verifyingAllPending, setVerifyingAllPending] = useState(false);
  const {
    buildUpdateSecurityAction,
    buildUpdateTarget,
    getUpdateCampaignFollowUp,
    getUpdateVerifyFollowUp,
    maybeNotifyUpdateResult,
  } = useUpdateFollowUps({ projectId, normalizedUrl });
  const recordUpdateTimelineEvent = useUpdateTimelineRecorder({
    loadUpdateHistory,
    normalizedUrl,
    projectId,
    projectPath,
  });

  // Clear in-flight verify indicators when the project scope changes, adjusting
  // state during render instead of via an effect.
  const verifyScope = `${normalizedUrl}:${projectId}:${projectPath ?? ""}`;
  const [renderedVerifyScope, setRenderedVerifyScope] = useState(verifyScope);
  if (renderedVerifyScope !== verifyScope) {
    setRenderedVerifyScope(verifyScope);
    setVerifyingUpdateKey(null);
    setVerifyingPendingId(null);
    setVerifyingAllPending(false);
  }

  const handleVerifyUpdate = useCallback(
    async (update: PackageUpdate) => {
      if (!projectPath) return;
      const key = `${update.ecosystem}:${update.name}`;
      const target = buildUpdateTarget(key);
      addJob({
        id: `updates-verify:${projectId}:${key}`,
        type: "sync",
        label: "Verify package update",
        scopeLabel: `${projectName} \u2022 ${hostname}`,
        detail: formatPackageUpdateSummary(update),
        target,
      });
      setVerifyingUpdateKey(key);
      try {
        const beforeSummary = buildUpdateQueueSummary(report?.updates ?? []);
        const nextReport = await loadReport({ showToast: false });
        if (!nextReport) return;
        const afterSummary = buildUpdateQueueSummary(nextReport.updates);

        const stillPresent = nextReport.updates.some(
          (candidate) => candidate.name === update.name && candidate.ecosystem === update.ecosystem,
        );
        const nextUpdate =
          nextReport.updates.find(
            (candidate) =>
              candidate.name === update.name && candidate.ecosystem === update.ecosystem,
          ) ?? update;
        const diffSummary = [
          `Pending updates ${beforeSummary.total} -> ${afterSummary.total}`,
          `Security updates ${beforeSummary.security} -> ${afterSummary.security}`,
        ].join(" | ");

        if (stillPresent) {
          markUpdateStillPending(projectPath, update);
          toast.warning("Update still pending", diffSummary);
          completeJob(`updates-verify:${projectId}:${key}`, {
            label: "Update still pending",
            detail: `${update.name} \u2022 ${diffSummary}`,
            target,
          });
          maybeNotifyUpdateResult({
            id: `updates-verify:${projectId}:${key}:pending`,
            title: "Update still pending",
            body: `${update.name} is still waiting after verification. ${diffSummary}`,
            target,
            secondaryAction: buildUpdateSecurityAction(nextUpdate),
          });
        } else {
          const followUp = getUpdateVerifyFollowUp(update, nextReport.updates);
          markUpdateVerified(projectPath, update);
          toast.success(
            "Update verified",
            [diffSummary, followUp.nextSummary].filter(Boolean).join(" | "),
          );
          completeJob(`updates-verify:${projectId}:${key}`, {
            label: "Update verified",
            detail: [`${update.name}`, diffSummary, followUp.nextSummary]
              .filter(Boolean)
              .join(" | "),
            target: followUp.target,
          });
          maybeNotifyUpdateResult({
            id: `updates-verify:${projectId}:${key}:verified`,
            title: "Update verified",
            body: [`${update.name} cleared from Updates.`, diffSummary, followUp.nextSummary]
              .filter(Boolean)
              .join(" "),
            target: followUp.target,
            secondaryAction: followUp.secondaryAction,
          });
          recordUpdateTimelineEvent({
            sourceId: `updates-verify:${projectId}:${key}:verified:${Date.now()}`,
            title: "1 Update Applied",
            summary: [
              `${formatPackageUpdateSummary(update)} cleared from Updates.`,
              followUp.nextSummary ?? "Everything in Updates is verified for now.",
            ].join(" "),
            target:
              afterSummary.total > 0
                ? followUp.target
                : {
                    page: "updates",
                    projectId,
                    url: normalizedUrl,
                  },
            itemLabel: formatPackageUpdateSummary(update),
            verifiedLabel: formatPackageUpdateSummary(update),
            nextItemLabel: followUp.nextRelatedUpdate
              ? formatPackageUpdateSummary(followUp.nextRelatedUpdate)
              : null,
            appliedUpdates: [update],
            statusBefore: "Pending",
            statusAfter: "Verified",
            remainingUpdates: afterSummary.total,
            securityUpdates: afterSummary.security,
            remainingBreakdown: afterSummary.breakdown,
            workflowLabel:
              afterSummary.total > 0 ? "Exact package verified" : "Dependencies cleared",
          });
        }
      } catch (e) {
        failJob(`updates-verify:${projectId}:${key}`, {
          label: "Update verification failed",
          detail: `${update.name} \u2022 ${userFacingError(e, VERIFICATION_FALLBACK)}`,
          target,
        });
        toast.error("Verification failed", userFacingError(e, VERIFICATION_FALLBACK));
      } finally {
        setVerifyingUpdateKey((current) => (current === key ? null : current));
      }
    },
    [
      buildUpdateSecurityAction,
      buildUpdateTarget,
      getUpdateVerifyFollowUp,
      hostname,
      loadReport,
      maybeNotifyUpdateResult,
      normalizedUrl,
      projectId,
      projectName,
      projectPath,
      recordUpdateTimelineEvent,
      report,
      toast,
    ],
  );

  const handleVerifyPendingEntry = useCallback(
    async (entry: PendingVerificationEntry) => {
      const currentReport = reportRef.current;
      const previousUpdates = currentReport?.updates ?? [];
      const beforeSummary = buildUpdateQueueSummary(currentReport?.updates ?? []);
      addJob({
        id: `updates-pending:${projectId}:${entry.itemId}`,
        type: "sync",
        label: "Verify package update",
        scopeLabel: `${projectName} \u2022 ${hostname}`,
        detail: entry.label,
        target: buildUpdateTarget(entry.itemId),
      });
      setVerifyingPendingId(entry.id);
      try {
        const nextReport = await loadReport({ showToast: false });
        if (!nextReport) return;
        const afterSummary = buildUpdateQueueSummary(nextReport.updates);
        const previousUpdate = findPackageUpdateByItemId(previousUpdates, entry.itemId);
        const currentUpdate = findPackageUpdateByItemId(nextReport.updates, entry.itemId);
        const target = buildUpdateTarget(entry.itemId);
        resolvePendingVerification(entry.id);
        const followUp =
          !currentUpdate && previousUpdate
            ? getUpdateVerifyFollowUp(previousUpdate, nextReport.updates)
            : null;
        toast.success(
          "Dependency re-check complete",
          [
            entry.label,
            `Pending updates ${beforeSummary.total} -> ${afterSummary.total}`,
            `Security updates ${beforeSummary.security} -> ${afterSummary.security}`,
            followUp?.nextSummary,
          ]
            .filter(Boolean)
            .join(" | "),
        );
        completeJob(`updates-pending:${projectId}:${entry.itemId}`, {
          label: currentUpdate ? "Update still pending" : "Update verified",
          detail: [
            entry.label,
            `Pending updates ${beforeSummary.total} -> ${afterSummary.total}`,
            `Security updates ${beforeSummary.security} -> ${afterSummary.security}`,
            followUp?.nextSummary,
          ]
            .filter(Boolean)
            .join(" | "),
          target: followUp?.target ?? target,
        });
        if (!currentUpdate) {
          recordUpdateTimelineEvent({
            sourceId: `updates-pending:${projectId}:${entry.itemId}:verified:${Date.now()}`,
            title: "1 Update Applied",
            summary: [
              `${entry.label} cleared from Updates.`,
              followUp?.nextSummary ?? "Everything in Updates is verified for now.",
            ].join(" "),
            target:
              afterSummary.total > 0
                ? (followUp?.target ?? target)
                : {
                    page: "updates",
                    projectId,
                    url: normalizedUrl,
                  },
            itemLabel: entry.label,
            verifiedLabel: entry.label,
            nextItemLabel: followUp?.nextRelatedUpdate
              ? formatPackageUpdateSummary(followUp.nextRelatedUpdate)
              : null,
            appliedUpdates: previousUpdate ? [previousUpdate] : null,
            statusBefore: "Pending",
            statusAfter: "Verified",
            remainingUpdates: afterSummary.total,
            securityUpdates: afterSummary.security,
            remainingBreakdown: afterSummary.breakdown,
            workflowLabel:
              afterSummary.total > 0 ? "Exact package verified" : "Dependencies cleared",
          });
        }
      } catch (error) {
        failJob(`updates-pending:${projectId}:${entry.itemId}`, {
          label: "Update verification failed",
          detail: `${entry.label} \u2022 ${userFacingError(error, VERIFICATION_FALLBACK)}`,
          target: buildUpdateTarget(entry.itemId),
        });
        toast.error("Verification failed", userFacingError(error, VERIFICATION_FALLBACK));
      } finally {
        setVerifyingPendingId((current) => (current === entry.id ? null : current));
      }
    },
    [
      buildUpdateTarget,
      getUpdateVerifyFollowUp,
      hostname,
      loadReport,
      normalizedUrl,
      projectId,
      projectName,
      recordUpdateTimelineEvent,
      reportRef,
      toast,
    ],
  );

  const handleVerifyAllPending = useCallback(async () => {
    if (pendingUpdateEntries.length === 0) return;
    const currentReport = reportRef.current;
    const previousUpdates = currentReport?.updates ?? [];
    const beforeSummary = buildUpdateQueueSummary(currentReport?.updates ?? []);
    const jobId = `updates-pending-all:${projectId}`;
    const leadPendingEntry = pendingUpdateEntries[0];
    const initialCampaign = buildUpdateCampaignCopy({
      totalCount: pendingUpdateEntries.length,
      leadLabel: leadPendingEntry?.label ?? "the strongest package update",
      leadSummary: leadPendingEntry?.label ?? undefined,
      mode: "verify",
    });
    addJob({
      id: jobId,
      type: "sync",
      label: pendingUpdateEntries.length > 1 ? "Verify package updates" : "Verify package update",
      scopeLabel: `${projectName} \u2022 ${hostname}`,
      detail: initialCampaign.detail,
      target: leadPendingEntry
        ? buildUpdateTarget(leadPendingEntry.itemId)
        : {
            page: "updates",
            projectId,
            url: normalizedUrl,
          },
    });
    setVerifyingAllPending(true);
    try {
      const nextReport = await loadReport({ showToast: false });
      if (!nextReport) return;
      const afterSummary = buildUpdateQueueSummary(nextReport.updates);
      for (const entry of pendingUpdateEntries) {
        resolvePendingVerification(entry.id);
      }
      const followUp = getUpdateCampaignFollowUp(nextReport.updates);
      const nextTargetUpdate =
        followUp.target.page === "updates" && typeof followUp.target.itemId === "string"
          ? (nextReport.updates.find(
              (candidate) => `${candidate.ecosystem}:${candidate.name}` === followUp.target.itemId,
            ) ?? null)
          : null;
      const detail = [
        `Pending reminders ${pendingUpdateEntries.length}`,
        `Pending updates ${beforeSummary.total} -> ${afterSummary.total}`,
        `Security updates ${beforeSummary.security} -> ${afterSummary.security}`,
        followUp.detail,
      ].join(" | ");
      toast.success("Pending dependency checks verified", detail);
      completeJob(jobId, {
        label:
          afterSummary.total > 0
            ? "Continue dependency cleanup"
            : "Dependency verification complete",
        detail,
        target: followUp.target,
      });
      maybeNotifyUpdateResult({
        id: `${jobId}:complete`,
        title: followUp.title,
        body: detail,
        target: followUp.target,
        secondaryAction: followUp.secondaryAction,
      });
      const appliedUpdates = getClearedUpdates(previousUpdates, nextReport.updates);
      if (appliedUpdates.length > 0) {
        recordUpdateTimelineEvent({
          sourceId: `${jobId}:${Date.now()}`,
          title: `${appliedUpdates.length} Update${appliedUpdates.length === 1 ? "" : "s"} Applied`,
          summary: [
            `Verified ${pendingUpdateEntries.length} pending dependency reminder${pendingUpdateEntries.length === 1 ? "" : "s"}.`,
            followUp.detail,
          ].join(" "),
          target: followUp.target,
          itemLabel: leadPendingEntry?.label ?? null,
          nextItemLabel: nextTargetUpdate ? formatPackageUpdateSummary(nextTargetUpdate) : null,
          appliedUpdates,
          verifiedCount: appliedUpdates.length,
          remainingUpdates: afterSummary.total,
          securityUpdates: afterSummary.security,
          remainingBreakdown: afterSummary.breakdown,
          workflowLabel:
            afterSummary.total > 0 ? "Dependency cleanup continues" : "Dependencies cleared",
        });
      }
    } catch (error) {
      failJob(jobId, {
        label: "Dependency verification failed",
        detail: userFacingError(error, VERIFICATION_FALLBACK),
        target: leadPendingEntry
          ? buildUpdateTarget(leadPendingEntry.itemId)
          : {
              page: "updates",
              projectId,
              url: normalizedUrl,
            },
      });
      toast.error("Verification failed", userFacingError(error, VERIFICATION_FALLBACK));
    } finally {
      setVerifyingAllPending(false);
    }
  }, [
    buildUpdateTarget,
    getUpdateCampaignFollowUp,
    hostname,
    loadReport,
    maybeNotifyUpdateResult,
    normalizedUrl,
    pendingUpdateEntries,
    projectId,
    projectName,
    recordUpdateTimelineEvent,
    reportRef,
    toast,
  ]);

  return {
    handleVerifyAllPending,
    handleVerifyPendingEntry,
    handleVerifyUpdate,
    verifyingAllPending,
    verifyingPendingId,
    verifyingUpdateKey,
  };
}
