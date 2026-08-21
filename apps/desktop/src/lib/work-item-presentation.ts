import type { AppTarget } from "@/lib/app-targets";
import { withNormalizedTarget } from "@/lib/app-targets";
import { getOpenTargetLabel } from "@/lib/action-language";
import type { ProjectWorkSummary, WorkItemStatus } from "@/lib/project-summary-signals";

export type WorkSummaryBadgeKey =
  "regressed" | "working" | "blocked" | "launch-blockers" | "ignored";

interface PersistedWorkSummaryCue {
  key: WorkSummaryBadgeKey;
  label: string;
  sentence: string;
}

function getWorkItemStatusBadge(status: WorkItemStatus): {
  label: string;
  className: string;
} {
  switch (status) {
    case "regressed":
      return {
        label: "Regressed",
        className: "work-badge--regressed",
      };
    case "working":
      return {
        label: "Working",
        className: "work-badge--working",
      };
    case "blocked":
      return {
        label: "Blocked",
        className: "work-badge--blocked",
      };
    case "ignored":
      return {
        label: "Ignored",
        className: "work-badge--ignored",
      };
    case "snoozed":
      return {
        label: "Snoozed",
        className: "work-badge--ignored",
      };
    case "verified":
      return {
        label: "Verified",
        className: "work-badge--verified",
      };
    case "new":
    default:
      return {
        label: "New",
        className: "work-badge--new",
      };
  }
}

export function getWorkSummaryBadges(
  summary: ProjectWorkSummary,
): Array<{ key: string; label: string; className: string }> {
  const badges: Array<{ key: string; label: string; className: string }> = [];

  if (summary.regressedCount > 0) {
    badges.push({
      key: "regressed",
      label: `${summary.regressedCount} regressed`,
      className: getWorkItemStatusBadge("regressed").className,
    });
  }
  if (summary.workingCount > 0) {
    badges.push({
      key: "working",
      label: `${summary.workingCount} working`,
      className: getWorkItemStatusBadge("working").className,
    });
  }
  if (summary.blockedCount > 0) {
    badges.push({
      key: "blocked",
      label: `${summary.blockedCount} blocked`,
      className: getWorkItemStatusBadge("blocked").className,
    });
  }
  if (summary.launchBlockerCount > 0) {
    badges.push({
      key: "launch-blockers",
      label: `${summary.launchBlockerCount} launch blocker${summary.launchBlockerCount === 1 ? "" : "s"}`,
      className: "work-badge--regressed",
    });
  }
  if (summary.ignoredCount > 0) {
    badges.push({
      key: "ignored",
      label: `${summary.ignoredCount} ignored`,
      className: getWorkItemStatusBadge("ignored").className,
    });
  }

  return badges;
}

export function getWorkSummaryBadgeTarget(
  summary: ProjectWorkSummary,
  key: WorkSummaryBadgeKey,
): AppTarget | null {
  switch (key) {
    case "regressed":
      return summary.regressedAction?.target ?? null;
    case "working":
      return summary.workingAction?.target ?? null;
    case "blocked":
      return summary.blockedAction?.target ?? null;
    case "ignored":
      return summary.ignoredAction?.target ?? null;
    case "launch-blockers":
      return summary.launchBlockerAction?.target ?? null;
    default:
      return null;
  }
}

export function getWorkSummaryBadgeClassName(key: WorkSummaryBadgeKey): string {
  switch (key) {
    case "regressed":
      return getWorkItemStatusBadge("regressed").className;
    case "working":
      return getWorkItemStatusBadge("working").className;
    case "blocked":
      return getWorkItemStatusBadge("blocked").className;
    case "ignored":
      return getWorkItemStatusBadge("ignored").className;
    case "launch-blockers":
      return "work-badge--regressed";
    default:
      return getWorkItemStatusBadge("new").className;
  }
}

export function readPersistedWorkSummaryCue(
  detail: Record<string, unknown> | null,
): PersistedWorkSummaryCue | null {
  const key = detail?.workflow_key;
  const label = detail?.workflow_label;
  const sentence = detail?.workflow_sentence;
  if (
    (key === "regressed" ||
      key === "working" ||
      key === "blocked" ||
      key === "launch-blockers" ||
      key === "ignored") &&
    typeof label === "string" &&
    label.trim() &&
    typeof sentence === "string" &&
    sentence.trim()
  ) {
    return {
      key,
      label,
      sentence,
    };
  }
  return null;
}

interface WorkSummaryPriorityCue {
  key: WorkSummaryBadgeKey;
  label: string;
  className: string;
  target: AppTarget | null;
  sentence: string;
}

interface WorkflowFollowUpBannerModel {
  id: string;
  title: string;
  description: string;
  actionLabel: string;
  target: AppTarget;
  tone: "followup" | "urgent";
}

interface WorkflowFollowUpBannerOptions {
  scopeLabel?: string | null;
  targetOverride?: AppTarget | null;
}

export function getPrimaryWorkSummaryCue(
  summary: ProjectWorkSummary,
): WorkSummaryPriorityCue | null {
  const badges = getWorkSummaryBadges(summary);
  const badge = badges[0];
  if (!badge) return null;

  const key = badge.key as WorkSummaryBadgeKey;
  const count =
    key === "regressed"
      ? summary.regressedCount
      : key === "working"
        ? summary.workingCount
        : key === "blocked"
          ? summary.blockedCount
          : key === "launch-blockers"
            ? summary.launchBlockerCount
            : summary.ignoredCount;

  const sentence =
    key === "regressed"
      ? `Resume ${count} regressed item${count === 1 ? "" : "s"} next.`
      : key === "working"
        ? `${count} in-progress item${count === 1 ? " is" : "s are"} ready to resume.`
        : key === "blocked"
          ? `${count} blocked item${count === 1 ? " needs" : "s need"} a decision.`
          : key === "launch-blockers"
            ? `${count} launch blocker${count === 1 ? " is" : "s are"} still open.`
            : `${count} ignored item${count === 1 ? " remains" : "s remain"} parked.`;

  return {
    key,
    label: badge.label,
    className: badge.className,
    target: getWorkSummaryBadgeTarget(summary, key),
    sentence,
  };
}

export function buildWorkflowFollowUpBanner(
  cue: Pick<WorkSummaryPriorityCue, "key" | "sentence" | "target"> | null | undefined,
  options?: WorkflowFollowUpBannerOptions,
): WorkflowFollowUpBannerModel | null {
  if (!cue?.target) return null;
  if (cue.key === "blocked" || cue.key === "ignored") return null;

  const target = withNormalizedTarget(options?.targetOverride ?? cue.target);
  const title =
    cue.key === "launch-blockers"
      ? "Launch still needs attention"
      : cue.key === "regressed"
        ? "A regression needs attention"
        : cue.key === "working"
          ? "Pick up where you left off"
          : "Recommended next step";
  const description = options?.scopeLabel ? `${options.scopeLabel}: ${cue.sentence}` : cue.sentence;

  return {
    id: `${cue.key}:${JSON.stringify(target)}`,
    title,
    description,
    actionLabel: getOpenTargetLabel(target),
    target,
    tone: cue.key === "regressed" || cue.key === "launch-blockers" ? "urgent" : "followup",
  };
}

export function getWorkSummaryFollowUpBanner(
  summary: ProjectWorkSummary,
  options?: WorkflowFollowUpBannerOptions,
): WorkflowFollowUpBannerModel | null {
  return buildWorkflowFollowUpBanner(getPrimaryWorkSummaryCue(summary), options);
}
