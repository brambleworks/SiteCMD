import { useCallback } from "react";

import type { AppTarget } from "@/lib/app-targets";
import { useDesktopPrefs } from "@/lib/desktop-prefs";
import { sendActionableDesktopNotification } from "@/lib/actionable-notifications";
import {
  buildNotificationActions,
  buildOpenTargetNotificationAction,
  type NotificationFollowUpAction,
} from "@/lib/notification-actions";
import {
  buildUpdateCampaignCopy,
  findNextRelatedUpdate,
  findStrongestPackageUpdate,
  formatPackageUpdateSummary,
  getPackageUpdateSourceLabel,
} from "@/lib/update-priority";
import type { PackageUpdate } from "@/lib/types";
import { countSecurityUpdates } from "@/lib/update-summary";

interface UseUpdateFollowUpsOptions {
  projectId: number;
  normalizedUrl: string;
}

interface NotifyUpdateResultOptions {
  id: string;
  title: string;
  body: string;
  target: AppTarget;
  secondaryAction?: NotificationFollowUpAction | null;
}

export function useUpdateFollowUps({ projectId, normalizedUrl }: UseUpdateFollowUpsOptions) {
  const { prefs: desktopPrefs } = useDesktopPrefs();

  const buildUpdateTarget = useCallback(
    (itemId?: string | null): AppTarget => ({
      page: "updates",
      projectId,
      url: normalizedUrl,
      itemId: itemId ?? null,
    }),
    [normalizedUrl, projectId],
  );

  const buildUpdateSecurityAction = useCallback(
    (update: PackageUpdate): NotificationFollowUpAction | null => {
      if (!update.isSecurity) return null;
      return {
        id: "open-security-issues",
        label: "Open security issues",
        target: {
          page: "issues",
          projectId,
          url: normalizedUrl,
          focus: "security",
        },
      };
    },
    [normalizedUrl, projectId],
  );

  const getUpdateVerifyFollowUp = useCallback(
    (current: PackageUpdate, remainingUpdates: PackageUpdate[]) => {
      const nextRelatedUpdate = findNextRelatedUpdate(current, remainingUpdates);
      const nextSummary = nextRelatedUpdate
        ? `Next up: ${formatPackageUpdateSummary(nextRelatedUpdate)}${nextRelatedUpdate.source === current.source ? ` • ${getPackageUpdateSourceLabel(current)}` : ""}`
        : null;
      return {
        nextRelatedUpdate,
        target: nextRelatedUpdate
          ? buildUpdateTarget(`${nextRelatedUpdate.ecosystem}:${nextRelatedUpdate.name}`)
          : buildUpdateTarget(`${current.ecosystem}:${current.name}`),
        nextSummary,
        secondaryAction: buildUpdateSecurityAction(nextRelatedUpdate ?? current),
      };
    },
    [buildUpdateSecurityAction, buildUpdateTarget],
  );

  const getUpdateCampaignFollowUp = useCallback(
    (remainingUpdates: PackageUpdate[]) => {
      const strongestRemaining = findStrongestPackageUpdate(remainingUpdates);
      if (!strongestRemaining) {
        return {
          title: "Dependency verification complete",
          detail: "Everything in Updates is verified for now.",
          target: {
            page: "updates" as const,
            projectId,
            url: normalizedUrl,
          },
          secondaryAction: null as NotificationFollowUpAction | null,
        };
      }

      const campaignCopy = buildUpdateCampaignCopy({
        totalCount: remainingUpdates.length,
        securityCount: countSecurityUpdates(remainingUpdates),
        leadLabel: formatPackageUpdateSummary(strongestRemaining),
        leadSummary: formatPackageUpdateSummary(strongestRemaining),
        leadSourceLabel: getPackageUpdateSourceLabel(strongestRemaining),
        mode: "fix",
      });

      return {
        title: campaignCopy.title,
        detail: campaignCopy.detail,
        target: buildUpdateTarget(`${strongestRemaining.ecosystem}:${strongestRemaining.name}`),
        secondaryAction: buildUpdateSecurityAction(strongestRemaining),
      };
    },
    [buildUpdateSecurityAction, buildUpdateTarget, normalizedUrl, projectId],
  );

  const maybeNotifyUpdateResult = useCallback(
    (options: NotifyUpdateResultOptions) => {
      if (
        !desktopPrefs.desktopNotifications ||
        typeof document === "undefined" ||
        document.visibilityState === "visible"
      ) {
        return;
      }

      void sendActionableDesktopNotification({
        id: options.id,
        title: options.title,
        body: options.body,
        clickTarget: options.target,
        actions: buildNotificationActions(
          buildOpenTargetNotificationAction("open-update", options.target),
          options.secondaryAction ?? null,
        ),
      }).catch(() => {});
    },
    [desktopPrefs.desktopNotifications],
  );

  return {
    buildUpdateSecurityAction,
    buildUpdateTarget,
    getUpdateCampaignFollowUp,
    getUpdateVerifyFollowUp,
    maybeNotifyUpdateResult,
  };
}
