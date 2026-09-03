import { Page, Text, View } from "@/lib/react-pdf-browser";
import { PDF_MUTED, PDF_SEVERITY } from "./report-pdf-colors";
import {
  buildScoreReconciliation,
  formatReportDate,
  scoreColor,
  severityColor,
  type CodeScanSummary,
  type ReportData,
  type ReportIssue,
} from "./report-pdf-model";
import { styles } from "./report-pdf-styles";

export function ReportTitlePage({ data, title }: { data: ReportData; title: string }) {
  const { branding, siteScore } = data;

  return (
    <Page size="A4" style={styles.page}>
      <View style={styles.titlePage}>
        <Text style={styles.siteUrl}>{data.siteUrl}</Text>
        <Text style={styles.reportTitle}>{title}</Text>
        <Text style={styles.periodLabel}>{data.periodLabel}</Text>

        <View style={[styles.scoreCircle, { backgroundColor: scoreColor(siteScore.currentScore) }]}>
          <Text style={styles.scoreValue}>{siteScore.currentScore}</Text>
        </View>

        <Text style={styles.generatedAt}>Generated {formatReportDate(data.generatedAt)}</Text>
        {branding.companyName ? (
          <Text style={styles.companyName}>{branding.companyName}</Text>
        ) : null}
        {branding.clientName ? (
          <Text style={{ fontSize: 10, color: "#888", marginTop: 2 }}>
            Prepared for {branding.clientName}
          </Text>
        ) : null}
      </View>
      <Footer data={data} />
    </Page>
  );
}

export function ExecutiveSummaryPage({ data }: { data: ReportData }) {
  const { categories, codeScan, siteScore } = data;

  return (
    <Page size="A4" style={styles.page}>
      <Text style={styles.sectionTitle}>Executive Summary</Text>
      <View style={styles.summaryRow}>
        <SummaryBox
          label="SiteCMD Score"
          value={siteScore.currentScore}
          color={scoreColor(siteScore.currentScore)}
        />
        <SummaryBox
          label="Critical"
          value={siteScore.issuesCritical}
          color={PDF_SEVERITY.critical}
        />
        <SummaryBox label="High" value={siteScore.issuesHigh} color={PDF_SEVERITY.high} />
        <SummaryBox label="Medium" value={siteScore.issuesMedium} color={PDF_SEVERITY.medium} />
        <SummaryBox label="Low" value={siteScore.issuesLow} />
      </View>
      <Text style={{ fontSize: 10, color: "#444", lineHeight: 1.6 }}>
        This report covers the SiteCMD assessment of {data.siteUrl} for the{" "}
        {data.periodLabel.toLowerCase()}
        {codeScan ? " and the latest linked Code Scan." : "."}{" "}
        {buildScoreReconciliation({
          siteScore: siteScore.currentScore,
          categoryCount: categories.length,
        })}
        {siteScore.issuesCritical > 0
          ? ` ${siteScore.issuesCritical} critical issue${siteScore.issuesCritical > 1 ? "s" : ""} require immediate attention.`
          : " No critical issues were found."}
        {data.resolvedCount > 0
          ? ` ${data.resolvedCount} issue${data.resolvedCount > 1 ? "s" : ""} were resolved during this period.`
          : ""}
        {codeScan
          ? ` Code issues add ${codeScan.criticalCount} critical and ${codeScan.highCount} high issues, led by ${codeScan.topDomain || "Code Scan"}.${codeScan.domainTrend ? ` ${codeScan.domainTrend}.` : ""}`
          : ""}
      </Text>
      {codeScan ? <CodeScanSnapshot codeScan={codeScan} /> : null}
      <Footer data={data} />
    </Page>
  );
}

export function CategoryBreakdownPage({ data }: { data: ReportData }) {
  return (
    <Page size="A4" style={styles.page}>
      <Text style={styles.sectionTitle}>Score Breakdown by Category</Text>
      <TableHeader
        columns={[
          { label: "Category", width: "40%" },
          { label: "Score", width: "20%", centered: true },
          { label: "Previous", width: "20%", centered: true },
          { label: "Issues", width: "20%", centered: true },
        ]}
      />
      {data.categories.map((cat, i) => (
        <View key={i} style={styles.tableRow}>
          <Text style={[styles.tableCell, { width: "40%" }]}>{cat.name}</Text>
          <Text
            style={[
              styles.tableCell,
              {
                width: "20%",
                textAlign: "center",
                color: scoreColor(cat.score),
                fontFamily: "Helvetica-Bold",
              },
            ]}>
            {cat.score}
          </Text>
          <Text style={[styles.tableCell, { width: "20%", textAlign: "center", color: "#999" }]}>
            {cat.previousScore != null ? String(cat.previousScore) : "-"}
          </Text>
          <Text style={[styles.tableCell, { width: "20%", textAlign: "center" }]}>
            {cat.issueCount}
          </Text>
        </View>
      ))}
      <Footer data={data} />
    </Page>
  );
}

export function CodeScanPage({ data }: { data: ReportData }) {
  const codeScan = data.codeScan;
  if (!codeScan) return null;

  return (
    <Page size="A4" style={styles.page} wrap>
      <Text style={styles.sectionTitle}>Code Scan</Text>
      <View style={styles.summaryRow}>
        <SummaryBox label="Issues" value={codeScan.issueCount} />
        <SummaryBox label="Critical" value={codeScan.criticalCount} color={PDF_SEVERITY.critical} />
        <SummaryBox label="High" value={codeScan.highCount} color={PDF_SEVERITY.high} />
        <SummaryBox label={codeScan.topDomain || "Top Domain"} value={codeScan.topDomainCount} />
      </View>
      <Text style={{ fontSize: 10, color: "#444", lineHeight: 1.6, marginBottom: 12 }}>
        Latest Code Scan checked {formatReportDate(codeScan.checkedAt)}.
        {codeScan.framework ? ` Framework: ${codeScan.framework}.` : ""}
        {codeScan.previousScore == null
          ? " This is the first Code Scan in this report window."
          : ""}
        {codeScan.domainTrend ? ` ${codeScan.domainTrend}.` : ""}
      </Text>
      <TableHeader
        columns={[
          { label: "Code Domain", width: "70%" },
          { label: "Issues", width: "30%", centered: true },
        ]}
      />
      {codeScan.domains.map((domain, i) => (
        <View key={i} style={styles.tableRow}>
          <Text style={[styles.tableCell, { width: "70%" }]}>{domain.name}</Text>
          <Text style={[styles.tableCell, { width: "30%", textAlign: "center" }]}>
            {domain.issueCount}
          </Text>
        </View>
      ))}
      {codeScan.topIssues.length > 0 ? (
        <View style={{ marginTop: 16 }}>
          <Text style={{ fontSize: 12, fontFamily: "Helvetica-Bold", marginBottom: 8 }}>
            Top Code Issues
          </Text>
          <IssueList issues={codeScan.topIssues.slice(0, 8)} />
        </View>
      ) : null}
      <Footer data={data} />
    </Page>
  );
}

export function TopIssuesPage({ data }: { data: ReportData }) {
  return (
    <Page size="A4" style={styles.page} wrap>
      <Text style={styles.sectionTitle}>{data.codeScan ? "Top Site Issues" : "Top Issues"}</Text>
      <IssueList issues={data.topIssues.slice(0, 20)} />
      <Footer data={data} />
    </Page>
  );
}

export function RecommendationsPage({ data }: { data: ReportData }) {
  const recommendationIssues: ReportIssue[] = [
    ...data.topIssues,
    ...(data.codeScan?.topIssues.map((issue) => ({
      ...issue,
      category: `Code Scan · ${issue.category}`,
    })) ?? []),
  ];
  const groups = [
    {
      title: "Fix Now",
      color: PDF_SEVERITY.critical,
      items: recommendationIssues.filter(
        (issue) => issue.severity === "critical" || issue.severity === "high",
      ),
    },
    {
      title: "Should Fix",
      color: PDF_SEVERITY.high,
      items: recommendationIssues.filter((issue) => issue.severity === "medium"),
    },
    {
      title: "Consider Fixing",
      color: PDF_MUTED,
      items: recommendationIssues.filter((issue) => issue.severity === "low"),
    },
  ].filter((group) => group.items.length > 0);

  return (
    <Page size="A4" style={styles.page} wrap>
      <Text style={styles.sectionTitle}>Recommendations</Text>
      <Text style={{ fontSize: 10, color: "#444", lineHeight: 1.6, marginBottom: 12 }}>
        Prioritized action items from live Web Scans and the linked Code Scan. Items are grouped by
        urgency so the highest-risk cleanup work lands first.
      </Text>

      {groups.map((group) => (
        <View key={group.title} style={{ marginBottom: 14 }}>
          <Text
            style={{
              fontSize: 11,
              fontFamily: "Helvetica-Bold",
              color: group.color,
              marginBottom: 8,
            }}>
            {group.title} ({group.items.length})
          </Text>
          <IssueList issues={group.items} />
        </View>
      ))}
      <Footer data={data} />
    </Page>
  );
}

export function AnalyticsReportPage({ data }: { data: ReportData }) {
  if (!data.analytics) return null;

  return (
    <Page size="A4" style={styles.page}>
      <Text style={styles.sectionTitle}>Analytics</Text>
      <View style={styles.summaryRow}>
        <SummaryBox label="Visitors" value={data.analytics.visitors.toLocaleString()} />
        <SummaryBox label="Pageviews" value={data.analytics.pageviews.toLocaleString()} />
        <SummaryBox label="Bounce Rate" value={`${data.analytics.bounceRate.toFixed(1)}%`} />
      </View>
      <Footer data={data} />
    </Page>
  );
}

export function UptimeReportPage({ data }: { data: ReportData }) {
  if (!data.uptime) return null;

  return (
    <Page size="A4" style={styles.page}>
      <Text style={styles.sectionTitle}>Uptime</Text>
      <View style={styles.summaryRow}>
        <SummaryBox label="Uptime" value={`${data.uptime.uptimePct.toFixed(2)}%`} />
        <SummaryBox label="Incidents" value={data.uptime.incidents} />
        <SummaryBox label="Avg Response" value={`${data.uptime.avgResponseMs} ms`} />
      </View>
      <Footer data={data} />
    </Page>
  );
}

export function DeploysReportPage({ data }: { data: ReportData }) {
  if (!data.deploys) return null;

  return (
    <Page size="A4" style={styles.page}>
      <Text style={styles.sectionTitle}>Deployments</Text>
      <View style={styles.summaryRow}>
        <SummaryBox label="Total" value={data.deploys.count} />
        <SummaryBox label="Recent Entries" value={data.deploys.recent.length} />
      </View>
      {data.deploys.recent.length > 0 ? (
        <>
          <TableHeader
            columns={[
              { label: "Date", width: "25%" },
              { label: "Commit", width: "50%" },
              { label: "Author", width: "25%" },
            ]}
          />
          {data.deploys.recent.map((deploy, i) => (
            <View key={`${deploy.date}-${deploy.message}-${i}`} style={styles.tableRow}>
              <Text style={[styles.tableCell, { width: "25%" }]}>
                {formatReportDate(deploy.date)}
              </Text>
              <Text style={[styles.tableCell, { width: "50%" }]}>{deploy.message}</Text>
              <Text style={[styles.tableCell, { width: "25%", color: "#666" }]}>
                {deploy.author || "-"}
              </Text>
            </View>
          ))}
        </>
      ) : null}
      <Footer data={data} />
    </Page>
  );
}

function CodeScanSnapshot({ codeScan }: { codeScan: CodeScanSummary }) {
  return (
    <View style={{ marginTop: 16 }}>
      <Text style={{ fontSize: 11, fontFamily: "Helvetica-Bold", marginBottom: 8 }}>
        Code Scan Snapshot
      </Text>
      <View style={styles.summaryRow}>
        <SummaryBox label="Issues" value={codeScan.issueCount} />
        <SummaryBox label="Critical" value={codeScan.criticalCount} color={PDF_SEVERITY.critical} />
        <SummaryBox label="High" value={codeScan.highCount} color={PDF_SEVERITY.high} />
        <SummaryBox label={codeScan.topDomain || "Top Domain"} value={codeScan.topDomainCount} />
      </View>
      <Text style={{ fontSize: 10, color: "#444", lineHeight: 1.6 }}>
        Latest Code Scan checked {formatReportDate(codeScan.checkedAt)}.
        {codeScan.framework ? ` Framework: ${codeScan.framework}.` : ""}
        {codeScan.previousScore == null
          ? " This is the first Code Scan in this report window."
          : ""}
        {codeScan.domainTrend ? ` ${codeScan.domainTrend}.` : ""}
      </Text>
    </View>
  );
}

function IssueList({ issues }: { issues: ReportIssue[] }) {
  return (
    <>
      {issues.map((issue, i) => (
        <View
          key={`${issue.title}-${i}`}
          style={[styles.issueRow, { borderLeftColor: severityColor(issue.severity) }]}
          wrap={false}>
          <Text style={styles.issueTitle}>{issue.title}</Text>
          <Text style={styles.issueDesc}>{issue.description}</Text>
          <View style={styles.issueMeta}>
            <Text style={[styles.badge, { backgroundColor: severityColor(issue.severity) }]}>
              {issue.severity.toUpperCase()}
            </Text>
            <Text style={{ fontSize: 8, color: "#888", paddingTop: 2 }}>{issue.category}</Text>
          </View>
        </View>
      ))}
    </>
  );
}

function SummaryBox({
  label,
  value,
  color,
}: {
  label: string;
  value: number | string;
  color?: string;
}) {
  return (
    <View style={styles.summaryBox}>
      <Text style={color ? [styles.summaryValue, { color }] : styles.summaryValue}>{value}</Text>
      <Text style={styles.summaryLabel}>{label}</Text>
    </View>
  );
}

function TableHeader({
  columns,
}: {
  columns: Array<{ label: string; width: string; centered?: boolean }>;
}) {
  return (
    <View style={styles.tableHeader}>
      {columns.map((column) => (
        <Text
          key={column.label}
          style={[
            styles.tableCell,
            {
              width: column.width,
              fontFamily: "Helvetica-Bold",
              textAlign: column.centered ? "center" : "left",
            },
          ]}>
          {column.label}
        </Text>
      ))}
    </View>
  );
}

function Footer({ data }: { data: ReportData }) {
  const { footerText, hideAttribution } = data.branding;

  return (
    <View style={styles.footer} fixed>
      <Text>{footerText}</Text>
      {!hideAttribution ? <Text>Generated by SiteCMD</Text> : null}
    </View>
  );
}
