import {
  countUnreadAlerts as countUnreadAlertsCmd,
  dismissAlert as dismissAlertCmd,
  getAlerts as getAlertsCmd,
  markAlertsViewedBulk as markAlertsViewedBulkCmd,
  markAlertUnread as markAlertUnreadCmd,
  markAlertViewed as markAlertViewedCmd,
} from "@/lib/commands";
import type { AlertRow, AlertFilter } from "./types";

export async function getAlerts(
  projectId: number,
  filter: AlertFilter = "unread",
  sinceMs?: number,
): Promise<AlertRow[]> {
  return getAlertsCmd({ projectId, filter, sinceMs });
}

export interface UnreadAlertCounts {
  total: number;
  critical: number;
}

export async function countUnreadAlerts(projectId: number): Promise<UnreadAlertCounts> {
  return countUnreadAlertsCmd({ projectId });
}

export async function markAlertViewed(alertId: number): Promise<void> {
  await markAlertViewedCmd({ alertId });
}

export async function markAlertUnread(alertId: number): Promise<void> {
  await markAlertUnreadCmd({ alertId });
}

export async function dismissAlert(alertId: number): Promise<void> {
  await dismissAlertCmd({ alertId });
}

export async function markAlertsViewedBulk(alertIds: number[]): Promise<void> {
  await markAlertsViewedBulkCmd({ alertIds });
}
