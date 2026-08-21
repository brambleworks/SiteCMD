import { Document } from "@/lib/react-pdf-browser";
import {
  AnalyticsReportPage,
  CategoryBreakdownPage,
  CodeScanPage,
  DeploysReportPage,
  ExecutiveSummaryPage,
  RecommendationsPage,
  ReportTitlePage,
  TopIssuesPage,
  UptimeReportPage,
} from "./ReportPDFSections";
import type { ReportData } from "./report-pdf-model";

export type { ReportData } from "./report-pdf-model";

export function ReportPDFDocument({ data }: { data: ReportData }) {
  const { branding, categories, codeScan, sections, topIssues } = data;
  const title = data.reportTitle || "Site & Code Report";
  const hasRecommendations = topIssues.length > 0 || (codeScan?.topIssues.length ?? 0) > 0;

  return (
    <Document title={title} author={branding.companyName || "SiteCMD"}>
      <ReportTitlePage data={data} title={title} />

      {sections.executiveSummary ? <ExecutiveSummaryPage data={data} /> : null}

      {sections.categoryBreakdown && categories.length > 0 ? (
        <CategoryBreakdownPage data={data} />
      ) : null}

      {sections.codeScan && codeScan ? <CodeScanPage data={data} /> : null}

      {sections.topIssues && topIssues.length > 0 ? <TopIssuesPage data={data} /> : null}

      {sections.recommendations && hasRecommendations ? <RecommendationsPage data={data} /> : null}

      {sections.analytics && data.analytics ? <AnalyticsReportPage data={data} /> : null}

      {sections.uptime && data.uptime ? <UptimeReportPage data={data} /> : null}

      {sections.deploys && data.deploys ? <DeploysReportPage data={data} /> : null}
    </Document>
  );
}
