import { getOpenTargetLabel } from "@/lib/action-language";
import type { AppTarget } from "@/lib/app-targets";
import { withNormalizedTarget } from "@/lib/app-targets";
import { buildWorkflowFollowUpBanner } from "@/lib/work-item-presentation";

export interface WorkflowCue {
  key: "regressed" | "working" | "blocked" | "launch-blockers" | "ignored";
  label: string;
  sentence: string;
  target: AppTarget | null;
}

export interface PostScanFollowUpBanner {
  id: string;
  title: string;
  description: string;
  actionLabel: string;
  tone: "followup" | "urgent";
  target: AppTarget;
}

function targetsMatch(a: AppTarget | null | undefined, b: AppTarget | null | undefined): boolean {
  if (!a || !b) return false;
  return JSON.stringify(withNormalizedTarget(a)) === JSON.stringify(withNormalizedTarget(b));
}

function mergeWorkflowTargetIntoPrimaryResult(
  workflowTarget: AppTarget,
  primaryTarget: AppTarget,
): AppTarget {
  if (workflowTarget.page !== "issues" || primaryTarget.page !== "issues") {
    return withNormalizedTarget(workflowTarget);
  }

  return withNormalizedTarget({
    ...primaryTarget,
    ...workflowTarget,
    projectId: workflowTarget.projectId ?? primaryTarget.projectId ?? null,
    url: workflowTarget.url ?? primaryTarget.url ?? null,
    scanId: workflowTarget.scanId ?? primaryTarget.scanId ?? null,
    sessionId: workflowTarget.sessionId ?? primaryTarget.sessionId ?? null,
    scanKind: workflowTarget.scanKind ?? primaryTarget.scanKind ?? null,
    focus: workflowTarget.focus ?? primaryTarget.focus ?? null,
    itemId: workflowTarget.itemId ?? primaryTarget.itemId ?? null,
    promptId: workflowTarget.promptId ?? primaryTarget.promptId ?? null,
    lane: workflowTarget.lane ?? primaryTarget.lane ?? null,
    reason: workflowTarget.reason ?? primaryTarget.reason ?? null,
    filePath: workflowTarget.filePath ?? primaryTarget.filePath ?? null,
  });
}

export function getPreferredPostScanTarget(
  workflowCue: WorkflowCue | null | undefined,
  primaryTarget: AppTarget,
): AppTarget {
  if (!workflowCue?.target) return withNormalizedTarget(primaryTarget);
  if (workflowCue.key === "blocked" || workflowCue.key === "ignored") {
    return withNormalizedTarget(primaryTarget);
  }

  const mergedWorkflowTarget = mergeWorkflowTargetIntoPrimaryResult(
    workflowCue.target,
    primaryTarget,
  );
  if (targetsMatch(mergedWorkflowTarget, primaryTarget)) {
    return withNormalizedTarget(primaryTarget);
  }

  if (mergedWorkflowTarget.page === "issues") {
    return mergedWorkflowTarget;
  }

  if (
    workflowCue.key === "regressed" ||
    workflowCue.key === "working" ||
    workflowCue.key === "launch-blockers"
  ) {
    return mergedWorkflowTarget;
  }

  return withNormalizedTarget(primaryTarget);
}

export function buildPostScanFollowUpBanner(
  workflowCue: WorkflowCue | null | undefined,
  chosenTarget: AppTarget,
  primaryTarget: AppTarget,
): PostScanFollowUpBanner | null {
  if (!workflowCue || targetsMatch(chosenTarget, primaryTarget)) return null;
  return buildWorkflowFollowUpBanner(workflowCue, {
    targetOverride: chosenTarget,
  });
}

export function getWorkflowNotificationFollowUpAction(
  workflowCue: Pick<WorkflowCue, "label" | "sentence" | "target"> | null | undefined,
  primaryTarget: AppTarget,
) {
  if (!workflowCue?.target || targetsMatch(workflowCue.target, primaryTarget)) {
    return null;
  }

  return {
    id: "resume-workflow",
    label: getOpenTargetLabel(workflowCue.target),
    target: workflowCue.target,
  };
}
