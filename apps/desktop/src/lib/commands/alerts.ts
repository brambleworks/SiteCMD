import { command } from "./invoke";
import type { AlertFilter, AlertRow, UnreadAlertCounts } from "@/generated/ipc-bindings";

export function getAlerts(args: {
  projectId: number;
  filter?: AlertFilter | null;
  sinceMs?: number | null;
}): Promise<AlertRow[]> {
  return command<AlertRow[]>("get_alerts", args);
}

export function markAlertViewed(args: { alertId: number }): Promise<void> {
  return command<void>("mark_alert_viewed", args);
}

export function markAlertUnread(args: { alertId: number }): Promise<void> {
  return command<void>("mark_alert_unread", args);
}

export function dismissAlert(args: { alertId: number }): Promise<void> {
  return command<void>("dismiss_alert", args);
}

export function countUnreadAlerts(args: { projectId: number }): Promise<UnreadAlertCounts> {
  return command<UnreadAlertCounts>("count_unread_alerts", args);
}

export function markAlertsViewedBulk(args: { alertIds: number[] }): Promise<void> {
  return command<void>("mark_alerts_viewed_bulk", args);
}
