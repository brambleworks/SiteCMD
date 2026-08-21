// Bridge the generated wire types and the richer report page model until those
// contracts are unified.
import {
  generateReportData as generateReportDataCmd,
  renderReportHtmlFromData as renderReportHtmlFromDataCmd,
} from "@/lib/commands";
import type { ReportData, SectionConfig } from "./reports-page-model";

export function generateReportData(args: {
  projectId: number;
  siteUrl: string;
  periodDays: number;
  sections?: SectionConfig | null;
}): Promise<ReportData> {
  return generateReportDataCmd(
    args as unknown as Parameters<typeof generateReportDataCmd>[0],
  ) as unknown as Promise<ReportData>;
}

export function renderReportHtmlFromData(args: { data: ReportData }): Promise<string> {
  return renderReportHtmlFromDataCmd(
    args as unknown as Parameters<typeof renderReportHtmlFromDataCmd>[0],
  );
}
