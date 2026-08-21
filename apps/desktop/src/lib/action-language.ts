import type { AppTarget } from "@/lib/app-targets";
import { isCodeScanFocus } from "@/lib/app-targets";
import type { ProjectWorkItem } from "@/lib/project-summary-signals";
import { CATEGORY_LABELS } from "@/lib/tokens";
import { getPageTargetLabel, getReasonTargetLabel } from "@/lib/target-action-labels";

type IssueLifecycleAction = "working" | "ignored" | "blocked" | "reopened";
type WorkQueueBucket = "resume" | "verify" | "fix" | "maintenance";
type CopyActionKind =
  | "fix-bundle"
  | "ai-task"
  | "commands"
  | "fix-plan"
  | "fix-steps"
  | "fix-prompt"
  | "proof-checklist"
  | "source-evidence"
  | "patch-prompt"
  | "prompt"
  | "command";

interface CopyActionLabelOptions {
  copied?: boolean;
  subject?: string | null;
}

interface SummaryTargetLabelOptions {
  itemCount?: number | null;
}

type TargetLike = Pick<
  AppTarget,
  "page" | "focus" | "scanKind" | "scanId" | "sessionId" | "reason" | "itemId"
>;

export function getOpenTargetLabel(target?: TargetLike | null): string {
  if (!target) return getPageTargetLabel("dashboard") ?? "Open Dashboard";

  const reasonLabel = getReasonTargetLabel(target.reason);
  if (reasonLabel) return reasonLabel;

  switch (target.page) {
    case "search-console":
      return getPageTargetLabel("search-console") ?? "Open Search & SEO";
    case "updates":
      if (target.itemId) {
        return "Open Package Update";
      }
      return getPageTargetLabel("updates") ?? "Open Updates";
    case "integrations":
      return getPageTargetLabel("integrations") ?? "Open Integrations";
    case "sites":
      return getPageTargetLabel("sites") ?? "Open Overview";
    case "events":
      return getPageTargetLabel("events") ?? "Open Activity";
    case "deploys":
      return getPageTargetLabel("deploys") ?? "Open Deploys";
    case "analytics":
      return getPageTargetLabel("analytics") ?? "Open Traffic";
    case "settings":
      return getPageTargetLabel("settings") ?? "Open Settings";
    case "issues":
      if (target.scanKind === "code" || isCodeScanFocus(target.focus)) {
        return "Open Code Scan";
      }
      if (target.scanId != null || target.sessionId != null) {
        return "Open Results";
      }
      return getPageTargetLabel("issues") ?? "Open Issues";
    case "dashboard":
    default:
      return getPageTargetLabel("dashboard") ?? "Open Dashboard";
  }
}

export function getSummaryTargetLabel(
  target?: TargetLike | null,
  options?: SummaryTargetLabelOptions,
): string {
  if (target?.page === "updates" && (options?.itemCount ?? 0) > 1) {
    return getPageTargetLabel("updates") ?? "Open Updates";
  }
  return getOpenTargetLabel(target);
}

export function getVerificationActionLabel(options?: { repeated?: boolean }): string {
  return options?.repeated ? "Verify again" : "Verify now";
}

export function getLifecycleActionLabel(action: IssueLifecycleAction): string {
  switch (action) {
    case "working":
      return "Mark Working";
    case "ignored":
      return "Ignore";
    case "blocked":
      return "Block";
    case "reopened":
    default:
      return "Reopen";
  }
}

export function getWebCategoryOpenLabel(category?: string | null): string {
  switch (category) {
    case "security":
      return "Open Security Issues";
    case "seo":
      return "Open Search & SEO";
    case "performance":
      return "Open Performance Results";
    case "accessibility":
      return "Open Accessibility Results";
    case "polish":
      return "Open Polish Results";
    case "compliance":
    case "legal":
      return `Open ${CATEGORY_LABELS.compliance} Results`;
    default:
      return getPageTargetLabel("issues") ?? "Open Issues";
  }
}

function buildCopyActionLabel(options: CopyActionLabelOptions | undefined, base: string): string {
  const copied = Boolean(options?.copied);
  const subject = options?.subject?.trim();
  const label = subject ? `${subject} ${base}` : base;
  return copied ? `Copied ${label}` : `Copy ${label}`;
}

export function getCopyActionLabel(kind: CopyActionKind, options?: CopyActionLabelOptions): string {
  const copied = Boolean(options?.copied);

  switch (kind) {
    case "fix-bundle":
      return buildCopyActionLabel(options, "Fix Bundle");
    case "ai-task":
      return buildCopyActionLabel(options, "AI Task");
    case "commands":
      return buildCopyActionLabel(options, "Commands");
    case "fix-plan":
      return buildCopyActionLabel(options, "Fix Plan");
    case "fix-steps":
      return buildCopyActionLabel(options, "Fix Steps");
    case "fix-prompt":
      return buildCopyActionLabel(options, "Fix Prompt");
    case "proof-checklist":
      return buildCopyActionLabel(options, "Verification Checklist");
    case "source-evidence":
      return buildCopyActionLabel(options, "Source Evidence");
    case "patch-prompt":
      return buildCopyActionLabel(options, "Patch Prompt");
    case "command":
      return buildCopyActionLabel(options, "Command");
    case "prompt":
    default:
      return copied ? "Copied Prompt" : "Copy Prompt";
  }
}

export function getWorkQueueActionLabel(
  bucket: WorkQueueBucket,
  item: Pick<ProjectWorkItem, "kind" | "status" | "target">,
): string {
  switch (bucket) {
    case "resume":
      return item.status === "blocked" ? "Resolve Block" : "Resume";
    case "verify":
      return getVerificationActionLabel();
    case "fix":
      return getOpenTargetLabel(item.target);
    case "maintenance":
    default:
      return item.status === "blocked" ? "Resolve Block" : getOpenTargetLabel(item.target);
  }
}
