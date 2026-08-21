import type { ActionableDesktopNotificationAction } from "@/lib/actionable-notifications";
import type { AppTarget } from "@/lib/app-targets";
import { getOpenTargetLabel } from "@/lib/action-language";

export interface NotificationFollowUpAction {
  id: string;
  label: string;
  target?: AppTarget | null;
  filePath?: string | null;
}

export function buildOpenTargetNotificationAction(
  id: string,
  target: AppTarget,
): NotificationFollowUpAction {
  return {
    id,
    label: getOpenTargetLabel(target),
    target,
  };
}

function toAction(
  action: NotificationFollowUpAction | null | undefined,
): ActionableDesktopNotificationAction | null {
  if (!action) return null;
  return {
    id: action.id,
    label: action.label,
    target: action.target ?? null,
    filePath: action.filePath ?? null,
  };
}

export function buildNotificationActions(
  ...actions: Array<NotificationFollowUpAction | null | undefined>
): ActionableDesktopNotificationAction[] {
  return actions
    .map(toAction)
    .filter((action): action is ActionableDesktopNotificationAction => action != null);
}

export function buildScanResultNotificationActions(options: {
  primaryTarget: AppTarget;
  secondaryAction?: NotificationFollowUpAction | null;
}): ActionableDesktopNotificationAction[] {
  return buildNotificationActions(
    buildOpenTargetNotificationAction("open-results", options.primaryTarget),
    options.secondaryAction ?? null,
  );
}

export function buildFileWatchNotificationActions(options: {
  filePath?: string | null;
  verifyTarget?: AppTarget | null;
}): ActionableDesktopNotificationAction[] {
  return buildNotificationActions(
    options.filePath
      ? {
          id: "open-file",
          label: options.verifyTarget ? "Open changed file" : "Open File",
          filePath: options.filePath,
        }
      : null,
    options.verifyTarget
      ? {
          id: "verify-now",
          label: getOpenTargetLabel(options.verifyTarget),
          target: options.verifyTarget,
        }
      : null,
  );
}
