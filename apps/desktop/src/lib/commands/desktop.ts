import { command } from "./invoke";
import type {
  ActionableDesktopNotificationRequest,
  DesktopCommandResult,
  DesktopWatchRequest,
  DesktopWatchSignal,
} from "@/generated/ipc-bindings";

export function inspectDesktopWatchFiles(args: {
  requests: DesktopWatchRequest[];
}): Promise<DesktopWatchSignal[]> {
  return command<DesktopWatchSignal[]>("inspect_desktop_watch_files", args);
}

export function updateTraySummary(args: {
  attentionCount: number;
  pendingCount: number;
  promptCount: number;
  runningCount: number;
}): Promise<void> {
  return command<void>("update_tray_summary", args);
}

export function updateTrayScanStatus(args: {
  scanning: boolean;
  url?: string | null;
  pct?: number | null;
}): Promise<void> {
  return command<void>("update_tray_scan_status", args);
}

export function sendActionableDesktopNotification(args: {
  request: ActionableDesktopNotificationRequest;
}): Promise<void> {
  return command<void>("send_actionable_desktop_notification", args);
}

export function runProjectCommand(args: {
  projectPath: string;
  command: string;
}): Promise<DesktopCommandResult> {
  return command<DesktopCommandResult>("run_project_command", args);
}

export function openPathInEditor(args: { path: string }): Promise<void> {
  return command<void>("open_path_in_editor", args);
}

export function revealPath(args: { path: string }): Promise<void> {
  return command<void>("reveal_path", args);
}

export function openExternalUrl(args: { url: string }): Promise<void> {
  return command<void>("open_external_url", args);
}
