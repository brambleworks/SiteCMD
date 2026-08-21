import {
  useCallback,
  useState,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from "react";

import { verifyScanChecks } from "@/lib/commands";
import { recordSearchEvent } from "@/lib/event-writes";
import type { CategoryScore, CheckResult } from "@/lib/types";
import type { AppTarget } from "@/lib/app-targets";
import { addJob, completeJob, failJob } from "@/lib/jobs";
import {
  buildNotificationActions,
  buildOpenTargetNotificationAction,
} from "@/lib/notification-actions";
import { sendActionableDesktopNotification } from "@/lib/actionable-notifications";
import {
  resolvePendingVerification,
  type PendingVerificationEntry,
} from "@/lib/pending-verification";
import { getSeoFocusLabel } from "@/lib/seo-focus";
import {
  buildSeoCategoryScore,
  formatCheckStatus,
  inferSeoFocus,
  matchesSeoFocus,
  resolveVerifiedIssue,
} from "@/components/dashboard/search-console-page-model";
import { formatUrlHost } from "@/lib/utils";
import {
  countActionableCheckResults,
  countPassingCheckResults,
  isActionableCheckResult,
  isPassingCheckResult,
} from "@/lib/issues";

interface SearchToast {
  success: (title: string, message?: string) => void;
  warning: (title: string, message?: string) => void;
  error: (title: string, message?: string) => void;
}

interface ApplySeoVerificationSummary {
  allChecks: CheckResult[];
  previousIssueCount: number;
  nextIssueCount: number;
  previousPassedCount: number;
  nextPassedCount: number;
}

interface UseSearchConsoleVerificationActionsParams {
  desktopNotificationsEnabled: boolean;
  normalizedUrl: string;
  pendingSearchEntries: PendingVerificationEntry[];
  projectId: number;
  seoIssuesStateRef: MutableRefObject<CheckResult[]>;
  seoPassedChecksStateRef: MutableRefObject<CheckResult[]>;
  setSeoIssues: Dispatch<SetStateAction<CheckResult[]>>;
  setSeoPassedChecks: Dispatch<SetStateAction<CheckResult[]>>;
  setSeoScore: Dispatch<SetStateAction<CategoryScore | null>>;
  toast: SearchToast;
  url: string;
}

export function useSearchConsoleVerificationActions({
  desktopNotificationsEnabled,
  normalizedUrl,
  pendingSearchEntries,
  projectId,
  seoIssuesStateRef,
  seoPassedChecksStateRef,
  setSeoIssues,
  setSeoPassedChecks,
  setSeoScore,
  toast,
  url,
}: UseSearchConsoleVerificationActionsParams) {
  const [verifyingCheckId, setVerifyingCheckId] = useState<string | null>(null);
  const [verifyingPendingId, setVerifyingPendingId] = useState<string | null>(null);
  const [verifyingAllPending, setVerifyingAllPending] = useState(false);

  const buildSearchTarget = useCallback(
    (issue?: CheckResult | null, focus?: string | null): AppTarget => ({
      page: "search-console",
      projectId,
      url: normalizedUrl,
      itemId: issue?.checkId ?? null,
      focus: focus ?? (issue ? inferSeoFocus(issue) : null),
    }),
    [normalizedUrl, projectId],
  );

  const maybeNotifySearchResult = useCallback(
    (options: { id: string; title: string; body: string; target: AppTarget }) => {
      if (
        !desktopNotificationsEnabled ||
        typeof document === "undefined" ||
        document.visibilityState === "visible"
      ) {
        return;
      }

      try {
        void Promise.resolve(
          sendActionableDesktopNotification({
            id: options.id,
            title: options.title,
            body: options.body,
            clickTarget: options.target,
            actions: buildNotificationActions(
              buildOpenTargetNotificationAction("open-search", options.target),
            ),
          }),
        ).catch(() => {});
      } catch {
        // Notification delivery should never block the verify flow.
      }
    },
    [desktopNotificationsEnabled],
  );

  const applySeoVerificationResults = useCallback(
    (results: CheckResult[]): ApplySeoVerificationSummary => {
      const previousIssues = seoIssuesStateRef.current;
      const previousPassed = seoPassedChecksStateRef.current;
      const nextChecks = [...previousIssues, ...previousPassed].map((entry) => {
        if (!results.some((candidate) => candidate.checkId === entry.checkId)) {
          return entry;
        }
        return resolveVerifiedIssue(entry, results);
      });
      const seen = new Set(nextChecks.map((entry) => entry.checkId));
      for (const result of results) {
        if (!seen.has(result.checkId)) {
          nextChecks.push(result);
        }
      }
      const nextIssues = nextChecks.filter(
        (entry) => entry.category === "seo" && isActionableCheckResult(entry),
      );
      const nextPassed = nextChecks
        .filter((entry) => entry.category === "seo" && isPassingCheckResult(entry))
        .sort((a, b) => a.title.localeCompare(b.title));
      seoIssuesStateRef.current = nextIssues;
      seoPassedChecksStateRef.current = nextPassed;
      setSeoIssues(nextIssues);
      setSeoPassedChecks(nextPassed);
      setSeoScore(buildSeoCategoryScore([...nextIssues, ...nextPassed]));
      return {
        allChecks: [...nextIssues, ...nextPassed],
        previousIssueCount: previousIssues.length,
        nextIssueCount: nextIssues.length,
        previousPassedCount: previousPassed.length,
        nextPassedCount: nextPassed.length,
      };
    },
    [seoIssuesStateRef, seoPassedChecksStateRef, setSeoIssues, setSeoPassedChecks, setSeoScore],
  );

  const verifySeoChecks = useCallback(
    async (checkIds: string[]) => {
      const verification = await verifyScanChecks({
        projectId,
        url,
        checkIds,
      });
      return {
        verification,
        summary: applySeoVerificationResults(verification.results),
      };
    },
    [applySeoVerificationResults, projectId, url],
  );

  const handleVerifyIssue = useCallback(
    async (issue: CheckResult) => {
      setVerifyingCheckId(issue.checkId);
      try {
        const { verification, summary } = await verifySeoChecks([issue.checkId]);
        const verifiedIssue = resolveVerifiedIssue(issue, verification.results);
        const nextIssue = isPassingCheckResult(verifiedIssue)
          ? (summary.allChecks.find(
              (candidate) =>
                candidate.checkId !== verifiedIssue.checkId && isActionableCheckResult(candidate),
            ) ?? null)
          : verifiedIssue;
        const followUpFocus = nextIssue ? inferSeoFocus(nextIssue) : inferSeoFocus(verifiedIssue);
        const diffSummary = [
          `${formatCheckStatus(issue.status)} -> ${formatCheckStatus(verifiedIssue.status)}`,
          `SEO issues ${summary.previousIssueCount} -> ${summary.nextIssueCount}`,
          `Passed ${summary.previousPassedCount} -> ${summary.nextPassedCount}`,
        ].join(" | ");
        const eventTitle =
          verifiedIssue.status === "pass"
            ? `Search issue verified: ${verifiedIssue.title}`
            : `Search issue still open: ${verifiedIssue.title}`;
        const eventSummary =
          verifiedIssue.status === "pass"
            ? [
                `${verifiedIssue.title} cleared in Search & SEO.`,
                nextIssue ? `Next issue: ${nextIssue.title}.` : "No Search & SEO issues remain.",
                diffSummary,
              ].join(" ")
            : `${verifiedIssue.title} still needs attention in Search & SEO. ${diffSummary}`;
        void recordSearchEvent({
          projectId,
          title: eventTitle,
          summary: eventSummary,
          detail: JSON.stringify({
            page: "search-console",
            url: normalizedUrl,
            item_id: nextIssue?.checkId ?? null,
            item_label: verifiedIssue.title,
            verified_label: verifiedIssue.title,
            next_item_label: nextIssue?.title ?? null,
            status_before: formatCheckStatus(issue.status),
            status_after: formatCheckStatus(verifiedIssue.status),
            focus: followUpFocus,
            focus_label: followUpFocus ? getSeoFocusLabel(followUpFocus) : null,
            checked_count: 1,
            open_checks: summary.nextIssueCount,
            passed_checks: summary.nextPassedCount,
            workflow_label:
              verifiedIssue.status === "pass"
                ? nextIssue
                  ? "Search verification continues"
                  : "Search issue verified"
                : "Search issue still open",
            reason: "search-verification",
          }),
          sourceId: `search-verify:${projectId}:${issue.checkId}:${verifiedIssue.status === "pass" ? "verified" : "pending"}:${Date.now()}`,
          severity:
            verifiedIssue.status === "pass" && summary.nextIssueCount === 0 ? "info" : "warning",
        }).catch(() => {});
        if (verifiedIssue.status === "pass") {
          toast.success("Issue verified", diffSummary);
        } else {
          toast.warning("Still failing", diffSummary);
        }
      } catch (error) {
        toast.error("Verification failed", String(error));
      } finally {
        setVerifyingCheckId((current) => (current === issue.checkId ? null : current));
      }
    },
    [normalizedUrl, projectId, toast, verifySeoChecks],
  );

  const collectSeoChecksForEntry = useCallback(
    (entry: PendingVerificationEntry) => {
      const allChecks = [...seoIssuesStateRef.current, ...seoPassedChecksStateRef.current];
      const exactMatch = allChecks.find((issue) => issue.checkId === entry.itemId);
      if (exactMatch) return [exactMatch];
      if (entry.focus) {
        return allChecks.filter((issue) => matchesSeoFocus(issue, entry.focus));
      }
      return allChecks.filter(isActionableCheckResult);
    },
    [seoIssuesStateRef, seoPassedChecksStateRef],
  );

  const getPrimarySeoFollowUp = useCallback((checks: CheckResult[], focus?: string | null) => {
    const matching = focus ? checks.filter((issue) => matchesSeoFocus(issue, focus)) : checks;
    return matching.find(isActionableCheckResult) ?? matching[0] ?? null;
  }, []);

  const recordSearchTimelineEvent = useCallback(
    (options: {
      sourceId: string;
      title: string;
      summary: string;
      targetIssue?: CheckResult | null;
      itemLabel?: string | null;
      focus?: string | null;
      checkedCount?: number;
      openChecks: number;
      passedChecks: number;
      workflowLabel?: string | null;
      severity?: "info" | "warning";
    }) => {
      const focus =
        options.focus ?? (options.targetIssue ? inferSeoFocus(options.targetIssue) : null);
      void recordSearchEvent({
        projectId,
        title: options.title,
        summary: options.summary,
        detail: JSON.stringify({
          page: "search-console",
          url: normalizedUrl,
          item_id: options.targetIssue?.checkId ?? null,
          item_label: options.itemLabel ?? options.targetIssue?.title ?? null,
          focus,
          focus_label: focus ? getSeoFocusLabel(focus) : null,
          checked_count: options.checkedCount ?? null,
          open_checks: options.openChecks,
          passed_checks: options.passedChecks,
          workflow_label: options.workflowLabel ?? null,
          reason: "search-verification",
        }),
        sourceId: options.sourceId,
        severity: options.severity ?? (options.openChecks > 0 ? "warning" : "info"),
      }).catch(() => {});
    },
    [normalizedUrl, projectId],
  );

  const handleVerifyPendingEntry = useCallback(
    async (entry: PendingVerificationEntry) => {
      const matchingChecks = collectSeoChecksForEntry(entry);
      if (matchingChecks.length === 0) {
        resolvePendingVerification(entry.id);
        toast.success(
          "Verification reminder cleared",
          `${entry.label} no longer has matching SEO checks to re-run.`,
        );
        return;
      }

      const clusterLabel = entry.focus ? getSeoFocusLabel(entry.focus) || entry.label : entry.label;
      const checkIds = [...new Set(matchingChecks.map((issue) => issue.checkId))];
      const beforeClusterIssues = countActionableCheckResults(matchingChecks);
      const beforeClusterPassed = countPassingCheckResults(matchingChecks);
      const clusterScopeLabel = formatUrlHost(normalizedUrl);
      const jobId = `search-verify:${projectId}:${entry.focus ?? entry.itemId}`;
      addJob({
        id: jobId,
        type: "probes",
        label: "Verify Search & SEO checks",
        scopeLabel: clusterScopeLabel,
        detail: clusterLabel,
        target: buildSearchTarget(matchingChecks[0] ?? null, entry.focus ?? null),
      });
      setVerifyingPendingId(entry.id);
      try {
        const { summary } = await verifySeoChecks(checkIds);
        const nextCluster = summary.allChecks.filter((issue) =>
          entry.focus ? matchesSeoFocus(issue, entry.focus) : checkIds.includes(issue.checkId),
        );
        const afterClusterIssues = countActionableCheckResults(nextCluster);
        const afterClusterPassed = countPassingCheckResults(nextCluster);
        resolvePendingVerification(entry.id);
        const diffSummary = [
          clusterLabel,
          `Open checks ${beforeClusterIssues} -> ${afterClusterIssues}`,
          `Passed ${beforeClusterPassed} -> ${afterClusterPassed}`,
        ].join(" | ");
        const followUpIssue = getPrimarySeoFollowUp(nextCluster, entry.focus);
        const followUpTarget = buildSearchTarget(followUpIssue, entry.focus ?? null);
        const eventTitle =
          afterClusterIssues === 0
            ? `Search checks verified: ${clusterLabel}`
            : `Search checks still open: ${clusterLabel}`;
        const eventSummary =
          afterClusterIssues === 0
            ? `${clusterLabel} now passes in Search & SEO. ${diffSummary}`
            : `${clusterLabel} still needs attention in Search & SEO. ${diffSummary}`;
        completeJob(jobId, {
          label: afterClusterIssues === 0 ? "Search checks verified" : "Search checks still open",
          detail: diffSummary,
          target: followUpTarget,
        });
        maybeNotifySearchResult({
          id: `${jobId}:${afterClusterIssues === 0 ? "verified" : "pending"}`,
          title:
            afterClusterIssues === 0
              ? "Search checks verified"
              : "Search checks still need attention",
          body: diffSummary,
          target: followUpTarget,
        });
        recordSearchTimelineEvent({
          sourceId: `${jobId}:${afterClusterIssues === 0 ? "verified" : "pending"}:${Date.now()}`,
          title: eventTitle,
          summary: eventSummary,
          targetIssue: followUpIssue,
          itemLabel: clusterLabel,
          focus: entry.focus ?? null,
          checkedCount: checkIds.length,
          openChecks: afterClusterIssues,
          passedChecks: afterClusterPassed,
          workflowLabel:
            afterClusterIssues === 0 ? "Search cluster verified" : "Search verification continues",
          severity: afterClusterIssues === 0 ? "info" : "warning",
        });
        if (afterClusterIssues === 0) {
          toast.success("Search reminder verified", diffSummary);
        } else {
          toast.warning("Search checks still need attention", diffSummary);
        }
      } catch (error) {
        failJob(jobId, {
          detail: String(error),
          target: buildSearchTarget(matchingChecks[0] ?? null, entry.focus ?? null),
        });
        toast.error("Verification failed", String(error));
      } finally {
        setVerifyingPendingId((current) => (current === entry.id ? null : current));
      }
    },
    [
      buildSearchTarget,
      collectSeoChecksForEntry,
      getPrimarySeoFollowUp,
      maybeNotifySearchResult,
      normalizedUrl,
      projectId,
      recordSearchTimelineEvent,
      toast,
      verifySeoChecks,
    ],
  );

  const handleVerifyAllPending = useCallback(async () => {
    if (pendingSearchEntries.length === 0) return;
    const groups = new Map<string, PendingVerificationEntry[]>();
    for (const entry of pendingSearchEntries) {
      const key = entry.focus ?? entry.itemId;
      groups.set(key, [...(groups.get(key) ?? []), entry]);
    }
    const beforeIssues = seoIssuesStateRef.current.length;
    const beforePassed = seoPassedChecksStateRef.current.length;
    let verifiedGroups = 0;
    const scopeLabel = formatUrlHost(normalizedUrl);
    const firstEntry = pendingSearchEntries[0] ?? null;
    const initialChecks = firstEntry ? collectSeoChecksForEntry(firstEntry) : [];
    const jobId = `search-verify-all:${projectId}:${normalizedUrl}`;
    addJob({
      id: jobId,
      type: "probes",
      label: "Verify pending Search & SEO checks",
      scopeLabel,
      detail: `${groups.size} focus area${groups.size === 1 ? "" : "s"} to verify`,
      target: buildSearchTarget(initialChecks[0] ?? null, firstEntry?.focus ?? null),
    });
    setVerifyingAllPending(true);
    try {
      for (const entries of groups.values()) {
        const matchingChecks = collectSeoChecksForEntry(entries[0]);
        if (matchingChecks.length > 0) {
          const checkIds = [...new Set(matchingChecks.map((issue) => issue.checkId))];
          await verifySeoChecks(checkIds);
          verifiedGroups += 1;
        }
        for (const entry of entries) {
          resolvePendingVerification(entry.id);
        }
      }
      const nextIssue = getPrimarySeoFollowUp(seoIssuesStateRef.current);
      const followUpTarget = buildSearchTarget(nextIssue);
      const detail = [
        `Focus areas ${verifiedGroups}`,
        `SEO issues ${beforeIssues} -> ${seoIssuesStateRef.current.length}`,
        `Passed ${beforePassed} -> ${seoPassedChecksStateRef.current.length}`,
      ].join(" | ");
      const eventTitle =
        seoIssuesStateRef.current.length === 0
          ? "Pending Search & SEO checks verified"
          : "Search & SEO checks still open";
      const eventSummary =
        seoIssuesStateRef.current.length === 0
          ? `Verified ${verifiedGroups} Search & SEO focus areas. ${detail}`
          : [
              `Verified ${verifiedGroups} Search & SEO focus areas.`,
              nextIssue ? `Next issue: ${nextIssue.title}.` : null,
              detail,
            ]
              .filter(Boolean)
              .join(" ");
      completeJob(jobId, {
        label:
          seoIssuesStateRef.current.length === 0
            ? "Pending Search & SEO checks verified"
            : "Search & SEO checks still open",
        detail,
        target: followUpTarget,
      });
      maybeNotifySearchResult({
        id: `${jobId}:complete`,
        title:
          seoIssuesStateRef.current.length === 0
            ? "Pending Search & SEO checks verified"
            : "Search & SEO checks still need attention",
        body: detail,
        target: followUpTarget,
      });
      recordSearchTimelineEvent({
        sourceId: `${jobId}:complete:${Date.now()}`,
        title: eventTitle,
        summary: eventSummary,
        targetIssue: nextIssue,
        focus: nextIssue ? inferSeoFocus(nextIssue) : null,
        checkedCount: verifiedGroups,
        openChecks: seoIssuesStateRef.current.length,
        passedChecks: seoPassedChecksStateRef.current.length,
        workflowLabel:
          seoIssuesStateRef.current.length === 0
            ? "Search & SEO backlog cleared"
            : "Search verification continues",
        severity: seoIssuesStateRef.current.length === 0 ? "info" : "warning",
      });
      toast.success("Pending search checks verified", detail);
    } catch (error) {
      failJob(jobId, {
        detail: String(error),
        target: buildSearchTarget(initialChecks[0] ?? null, firstEntry?.focus ?? null),
      });
      toast.error("Verification failed", String(error));
    } finally {
      setVerifyingAllPending(false);
    }
  }, [
    buildSearchTarget,
    collectSeoChecksForEntry,
    getPrimarySeoFollowUp,
    maybeNotifySearchResult,
    normalizedUrl,
    pendingSearchEntries,
    projectId,
    recordSearchTimelineEvent,
    seoIssuesStateRef,
    seoPassedChecksStateRef,
    toast,
    verifySeoChecks,
  ]);

  return {
    handleVerifyAllPending,
    handleVerifyIssue,
    handleVerifyPendingEntry,
    verifyingAllPending,
    verifyingCheckId,
    verifyingPendingId,
  };
}
