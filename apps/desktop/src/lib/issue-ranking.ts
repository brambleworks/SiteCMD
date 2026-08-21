import { CODE_SCAN_DOMAIN_META, getCodeIssueDomain } from "@/lib/code-scan-domains";
import type { FixEffort, FixGuideMeta } from "@/lib/fix-guide-shared";
import {
  createSeverityCounts,
  isSeverity,
  severityRank,
  type SeverityCounts,
} from "@/lib/severity";
import type { CheckResult, CodeIssue, CodeScanDomain, IssueGroup, ScanCategory } from "@/lib/types";
import { coerceJsonRecord } from "@/lib/json-record";
import { getIssueConfidence } from "@/lib/issue-confidence";
import { scoreIssueImpact } from "@/lib/sitecmd-score";
import { formatUrlPathOrHost } from "@/lib/utils";

export type FixQueueSource = "web" | "code" | "alert";

export interface AlertItem {
  id: string;
  priority: "critical" | "warning" | "info";
  title: string;
  description: string;
  action: string;
  onClick?: () => void;
}

export type UnifiedFixIssue =
  | {
      kind: "web";
      id: string;
      issue: CheckResult;
      groupedIssues: CheckResult[];
      occurrenceCount: number;
      occurrenceLabels: string[];
      impact: number;
      sourceLabel: string;
      effort: FixEffort | null;
      effortMinutes: number | null;
      /** Canonical backend projection. Present for the active Issues surface. */
      group?: IssueGroup;
    }
  | {
      kind: "code";
      id: string;
      issue: CodeIssue;
      groupedIssues: CodeIssue[];
      occurrenceCount: number;
      occurrenceLabels: string[];
      impact: number;
      sourceLabel: string;
      effort: FixEffort | null;
      effortMinutes: number | null;
      /** Canonical backend projection. Present for the active Issues surface. */
      group?: IssueGroup;
    }
  | {
      kind: "alert";
      id: string;
      issue: AlertItem;
      impact: number;
      sourceLabel: string;
      effort: null;
      effortMinutes: null;
    };

type ScanUnifiedFixIssue = Extract<UnifiedFixIssue, { kind: "web" | "code" }>;

const ALERT_PRIORITY_IMPACT: Record<string, number> = {
  critical: 15,
  warning: 4,
  info: 0.5,
};

function titleCaseCategory(category: string): string {
  if (category === "seo") return "SEO";
  return category.charAt(0).toUpperCase() + category.slice(1);
}

type WebGroupingEntry = { checkId: string };
type CodeGroupingEntry = { checkId: string; severity: string };

function getSeverityOrder(severity: string): number {
  if (isSeverity(severity)) return severityRank(severity);
  if (severity === "warning") return 1;
  if (severity === "info") return 3;
  return severityRank("low");
}

function buildCodeGroupKey(issue: CodeIssue): string {
  return issue.checkId;
}

function buildCodeGroupKeyFromEntry(issue: CodeGroupingEntry): string {
  return issue.checkId;
}

export function countGroupedWebIssues(webIssues: readonly WebGroupingEntry[]): number {
  return new Set(webIssues.map((issue) => issue.checkId)).size;
}

export function countGroupedCodeIssues(codeIssues: readonly CodeGroupingEntry[]): number {
  return new Set(codeIssues.map((issue) => buildCodeGroupKeyFromEntry(issue))).size;
}

type SeverityCountable = { severity: string };

export function getGroupedSeverityCounts(
  webIssues: readonly (WebGroupingEntry & SeverityCountable)[],
  codeIssues: readonly (CodeGroupingEntry & SeverityCountable)[],
): SeverityCounts {
  const counts = createSeverityCounts();
  const bump = (sev: string) => {
    if (isSeverity(sev)) counts[sev] += 1;
  };

  const webBySeverity = new Map<string, string>();
  for (const issue of webIssues) {
    const existing = webBySeverity.get(issue.checkId);
    if (!existing || getSeverityOrder(issue.severity) < getSeverityOrder(existing)) {
      webBySeverity.set(issue.checkId, issue.severity);
    }
  }
  for (const sev of webBySeverity.values()) bump(sev);

  const codeKeys = new Map<string, string>();
  for (const issue of codeIssues) {
    const key = buildCodeGroupKeyFromEntry(issue);
    const existing = codeKeys.get(key);
    if (!existing || getSeverityOrder(issue.severity) < getSeverityOrder(existing)) {
      codeKeys.set(key, issue.severity);
    }
  }
  for (const sev of codeKeys.values()) bump(sev);

  return counts;
}

function formatWebOccurrenceLabel(issue: CheckResult): string | null {
  const raw = coerceJsonRecord(issue.rawData);
  if (!raw) return null;

  const candidateKeys = [
    "url",
    "pageUrl",
    "page_url",
    "path",
    "pathname",
    "route",
    "endpoint",
    "action",
  ];
  for (const key of candidateKeys) {
    const value = raw[key];
    if (typeof value !== "string" || !value.trim()) continue;
    return formatUrlPathOrHost(value);
  }

  return null;
}

function sortWebIssuesForGrouping(a: CheckResult, b: CheckResult): number {
  const severityDelta = getSeverityOrder(a.severity) - getSeverityOrder(b.severity);
  if (severityDelta !== 0) return severityDelta;
  return (formatWebOccurrenceLabel(a) ?? "").localeCompare(formatWebOccurrenceLabel(b) ?? "");
}

function sortCodeIssuesForGrouping(a: CodeIssue, b: CodeIssue): number {
  const severityDelta = getSeverityOrder(a.severity) - getSeverityOrder(b.severity);
  if (severityDelta !== 0) return severityDelta;
  const pathDelta = a.relativePath.localeCompare(b.relativePath);
  if (pathDelta !== 0) return pathDelta;
  return (a.line ?? Number.MAX_SAFE_INTEGER) - (b.line ?? Number.MAX_SAFE_INTEGER);
}

export function rankUnified(
  webIssues: CheckResult[],
  codeIssues: CodeIssue[],
  alerts: AlertItem[],
  webGuideMetaByCheckId: Record<string, FixGuideMeta | undefined>,
): UnifiedFixIssue[] {
  const groupedWebIssues = new Map<string, CheckResult[]>();
  for (const issue of webIssues) {
    const existing = groupedWebIssues.get(issue.checkId) ?? [];
    existing.push(issue);
    groupedWebIssues.set(issue.checkId, existing);
  }

  const webRanked: UnifiedFixIssue[] = Array.from(groupedWebIssues.entries()).map(
    ([checkId, groupedIssues]) => {
      const sortedIssues = [...groupedIssues].sort(sortWebIssuesForGrouping);
      const issue = sortedIssues[0];
      const occurrenceCount = sortedIssues.length;
      const statusImpact = issue.status === "warn" ? "warn" : "fail";
      const impact = Math.max(
        1,
        Math.round(
          scoreIssueImpact(
            issue.severity,
            getIssueConfidence(issue),
            statusImpact,
            occurrenceCount,
          ),
        ),
      );
      const guide = webGuideMetaByCheckId[checkId] ?? null;
      const occurrenceLabels = Array.from(
        new Set(
          sortedIssues
            .map((entry) => formatWebOccurrenceLabel(entry))
            .filter((value): value is string => Boolean(value)),
        ),
      );
      return {
        kind: "web" as const,
        id: `web:${checkId}`,
        issue,
        groupedIssues: sortedIssues,
        occurrenceCount,
        occurrenceLabels,
        impact,
        sourceLabel: titleCaseCategory(issue.category),
        effort: guide?.effort ?? null,
        effortMinutes: guide?.effortMinutes ?? null,
      };
    },
  );

  const groupedCodeIssues = new Map<string, CodeIssue[]>();
  for (const issue of codeIssues) {
    const key = buildCodeGroupKey(issue);
    const existing = groupedCodeIssues.get(key) ?? [];
    existing.push(issue);
    groupedCodeIssues.set(key, existing);
  }

  const codeRanked: UnifiedFixIssue[] = Array.from(groupedCodeIssues.entries()).map(
    ([key, groupedIssues]) => {
      const sortedIssues = [...groupedIssues].sort(sortCodeIssuesForGrouping);
      const issue = sortedIssues[0];
      const domain = getCodeIssueDomain(issue);
      const meta = CODE_SCAN_DOMAIN_META[domain];
      const occurrenceCount = sortedIssues.length;
      const impact = Math.max(
        1,
        Math.round(
          scoreIssueImpact(issue.severity, getIssueConfidence(issue), "fail", occurrenceCount),
        ),
      );
      const occurrenceLabels = sortedIssues.map(
        (entry) => `${entry.relativePath}${entry.line ? `:${entry.line}` : ""}`,
      );
      return {
        kind: "code" as const,
        id: `code-group:${key}`,
        issue,
        groupedIssues: sortedIssues,
        occurrenceCount,
        occurrenceLabels,
        impact,
        sourceLabel: meta?.shortLabel ?? "Code",
        effort: null,
        effortMinutes: null,
      };
    },
  );

  const alertRanked: UnifiedFixIssue[] = alerts.map((alert) => ({
    kind: "alert" as const,
    id: `alert:${alert.id}`,
    issue: alert,
    impact: ALERT_PRIORITY_IMPACT[alert.priority] ?? 1,
    sourceLabel: "Alert",
    effort: null,
    effortMinutes: null,
  }));

  return [...webRanked, ...codeRanked, ...alertRanked].sort((a, b) => {
    if (b.impact !== a.impact) return b.impact - a.impact;
    // Tiebreak by severity so colors stay grouped
    const aSev =
      a.kind === "alert"
        ? getSeverityOrder(a.issue.priority)
        : getSeverityOrder((a.issue as CheckResult | CodeIssue).severity);
    const bSev =
      b.kind === "alert"
        ? getSeverityOrder(b.issue.priority)
        : getSeverityOrder((b.issue as CheckResult | CodeIssue).severity);
    return aSev - bSev;
  });
}

const WEB_CATEGORIES = new Set<ScanCategory>([
  "security",
  "performance",
  "seo",
  "accessibility",
  "compliance",
  "config",
  "polish",
]);

const CODE_DOMAINS = new Set<CodeScanDomain>([
  "database",
  "ai-safety",
  "security",
  "architecture",
  "operations",
  "supply-chain",
  "ai-scaffolding",
]);

function parseDetail(value: string | null): Record<string, unknown> {
  if (!value) return {};
  try {
    return coerceJsonRecord(JSON.parse(value)) ?? {};
  } catch {
    return {};
  }
}

function groupWebCategory(group: IssueGroup): ScanCategory {
  const candidate =
    group.instances.find((instance) => instance.category)?.category ?? group.category;
  return WEB_CATEGORIES.has(candidate as ScanCategory) ? (candidate as ScanCategory) : "config";
}

function groupCodeDomain(group: IssueGroup): CodeScanDomain {
  const candidate = group.instances.find((instance) => instance.domain)?.domain;
  return candidate && CODE_DOMAINS.has(candidate) ? candidate : "architecture";
}

function groupSourceLabel(group: IssueGroup): string {
  const hasCode = group.instances.some((instance) => instance.source === "code_scan");
  const hasOther = group.instances.some((instance) => instance.source !== "code_scan");
  if (hasCode && hasOther) return "Web + Code";
  if (hasCode) return "Code";
  return titleCaseCategory(group.category);
}

/** Builds the active queue from canonical backend issue groups. */
export function rankIssueGroups(
  groups: IssueGroup[],
  webGuideMetaByCheckId: Record<string, FixGuideMeta | undefined> = {},
): ScanUnifiedFixIssue[] {
  return groups
    .filter((group) => group.status === "new" || group.status === "regressed")
    .map((group): ScanUnifiedFixIssue => {
      const codeOnly = group.instances.every((instance) => instance.source === "code_scan");
      const sourceLabel = groupSourceLabel(group);
      const impact = Math.max(1, Math.round(group.impactScore));

      if (codeOnly) {
        const domain = groupCodeDomain(group);
        const groupedIssues = group.instances.map((instance): CodeIssue => {
          const detail = parseDetail(instance.detailJson);
          const relativePath = instance.relativePath ?? "";
          return {
            id: instance.producerCheckId ?? instance.signalId,
            checkId: group.checkId,
            category: instance.category ?? group.category,
            domain,
            severity: instance.severity,
            title: instance.title,
            description: instance.description,
            relativePath,
            absolutePath:
              typeof detail.absolutePath === "string" ? detail.absolutePath : relativePath,
            line: instance.line,
            sourceExcerpt: typeof detail.sourceExcerpt === "string" ? detail.sourceExcerpt : null,
            evidence: typeof detail.evidence === "string" ? detail.evidence : null,
            whyNow: instance.whyItMatters ?? null,
            likelyFix: instance.producerFixPrompt ?? instance.fixPrompt ?? null,
            confidence: instance.confidence ?? "high",
            confidenceReason: instance.confidenceReason,
            verifyHint: typeof detail.verifyHint === "string" ? detail.verifyHint : null,
          };
        });
        const sortedIssues = groupedIssues.sort(sortCodeIssuesForGrouping);
        return {
          kind: "code",
          id: `issue-group:${group.checkId}`,
          issue: sortedIssues[0],
          groupedIssues: sortedIssues,
          occurrenceCount: group.instances.length,
          occurrenceLabels: group.instances.map((instance) =>
            instance.relativePath
              ? `${instance.relativePath}${instance.line ? `:${instance.line}` : ""}`
              : (instance.pageUrl ?? instance.source),
          ),
          impact,
          sourceLabel,
          effort: null,
          effortMinutes: null,
          group,
        };
      }

      const category = groupWebCategory(group);
      const groupedIssues = group.instances
        .filter((instance) => instance.source !== "code_scan")
        .map((instance): CheckResult => ({
          checkId: group.checkId,
          category,
          title: instance.title,
          description: instance.description,
          status: instance.checkStatus ?? "fail",
          severity: instance.severity,
          fixPrompt: instance.fixPrompt ?? null,
          manualFix: instance.manualFix ?? null,
          rawData: parseDetail(instance.detailJson),
          confidence: instance.confidence ?? "high",
          confidenceReason: instance.confidenceReason,
          whyItMatters: instance.whyItMatters,
        }));
      const sortedIssues = groupedIssues.sort(sortWebIssuesForGrouping);
      const guide = webGuideMetaByCheckId[group.checkId] ?? null;
      return {
        kind: "web",
        id: `issue-group:${group.checkId}`,
        issue: sortedIssues[0],
        groupedIssues: sortedIssues,
        occurrenceCount: group.instances.length,
        occurrenceLabels: group.instances.map(
          (instance) =>
            instance.pageUrl ?? instance.relativePath ?? instance.url ?? instance.source,
        ),
        impact,
        sourceLabel,
        effort: guide?.effort ?? null,
        effortMinutes: guide?.effortMinutes ?? null,
        group,
      };
    })
    .sort((left, right) => {
      if (right.impact !== left.impact) return right.impact - left.impact;
      return getSeverityOrder(left.issue.severity) - getSeverityOrder(right.issue.severity);
    });
}

export function findUnifiedByCheckId(
  ranked: UnifiedFixIssue[],
  checkId: string,
): UnifiedFixIssue | null {
  return (
    ranked.find(
      (item) =>
        item.id === `issue-group:${checkId}` ||
        item.id === `web:${checkId}` ||
        item.id === `code-group:${checkId}`,
    ) ?? null
  );
}
