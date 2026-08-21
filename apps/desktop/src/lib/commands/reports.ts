import { command } from "./invoke";
import type {
  ReportBranding,
  ReportData,
  ReportHistoryEntry,
  SectionConfig,
} from "@/generated/ipc-bindings";

export function generateReportData(args: {
  projectId: number;
  siteUrl: string;
  periodDays: number;
  branding?: ReportBranding | null;
  reportTitle?: string | null;
  sections?: SectionConfig | null;
}): Promise<ReportData> {
  return command<ReportData>("generate_report_data", args);
}

export function renderReportHtmlFromData(args: { data: ReportData }): Promise<string> {
  return command<string>("render_report_html_from_data", args);
}

export function saveReportHistory(args: {
  projectId: number;
  siteUrl: string;
  periodDays: number;
  reportTitle: string;
  outputFormat: string;
  brandingJson: string;
  sectionsJson: string;
  reportSummaryJson?: string | null;
}): Promise<number> {
  return command<number>("save_report_history", args);
}

export function getReportHistory(args: { projectId: number }): Promise<ReportHistoryEntry[]> {
  return command<ReportHistoryEntry[]>("get_report_history", args);
}

export function deleteReportHistory(args: { id: number }): Promise<void> {
  return command<void>("delete_report_history", args);
}
