import { command } from "./invoke";
import type {
  DashboardReferenceSignals,
  DashboardSnapshot,
  ProjectNavBadgeSnapshot,
  ProjectSignalSnapshot,
  TodayProjectWorkSummary,
} from "@/generated/ipc-bindings";

export function getDashboardSnapshot(args: {
  projectId: number;
  url: string;
  forceRefresh?: boolean;
}): Promise<DashboardSnapshot> {
  return command<DashboardSnapshot>("get_dashboard_snapshot", args);
}

export function getDashboardReferenceSignals(args: {
  projectId: number;
  url: string;
  includePsi?: boolean;
}): Promise<DashboardReferenceSignals> {
  return command<DashboardReferenceSignals>("get_dashboard_reference_signals", args);
}

export function getProjectSignalSnapshot(args: {
  projectId: number;
  url?: string | null;
  forceRefresh?: boolean;
  includeCodeScanDetail?: boolean;
}): Promise<ProjectSignalSnapshot> {
  return command<ProjectSignalSnapshot>("get_project_signal_snapshot", args);
}

export function getProjectNavBadgeSnapshot(args: {
  projectId: number;
  url: string;
  forceRefresh?: boolean;
}): Promise<ProjectNavBadgeSnapshot> {
  return command<ProjectNavBadgeSnapshot>("get_project_nav_badge_snapshot", args);
}

export function getAllProjectsWorkSummary(args: {
  forceRefresh?: boolean;
}): Promise<TodayProjectWorkSummary[]> {
  return command<TodayProjectWorkSummary[]>("get_all_projects_work_summary", args);
}

export function invalidateProjectSignalSnapshot(args: {
  projectId: number;
  url?: string | null;
}): Promise<void> {
  return command<void>("invalidate_project_signal_snapshot", args);
}

export function dismissFirstScanBanner(args: { projectId: number }): Promise<void> {
  return command<void>("dismiss_first_scan_banner", args);
}
