import {
  FULL_SCAN_MERGE_WINDOW_MS,
  formatUpdateBreakdown,
  normalizeActivityFeedEvents,
  toEpoch,
} from "@/lib/activity-feed";
import { SCAN_LABELS } from "@/lib/scan-labels";
import type { CodeScanSummary, ScanResult, SiteEvent } from "@/lib/types";
import type { UpdateQueueBreakdown } from "@/lib/update-summary";

type ActivityColor = "default" | "amber" | "red" | "green";
type ActivityTarget = "deploys" | "issues" | "code-scan" | "updates" | "events";

interface DashboardActivityItem {
  id: string;
  label: string;
  value: string;
  valueColor?: ActivityColor;
  eventType?: string;
  source?: string;
  occurredAt: string;
  target: ActivityTarget;
  parsedDetail?: Record<string, unknown> | null;
}

interface DashboardActivityInputs {
  latestDeploy: {
    name: string;
    conclusion: string | null;
    status: string;
    updatedAt: string;
  } | null;
  commitsSinceLastScan: Array<{ timestamp: string }>;
  latestWebScan: ScanResult | null;
  webIssueCount: number;
  latestCodeScan: CodeScanSummary | null;
  updatesCheckedAt: string | null;
  updateBreakdown: UpdateQueueBreakdown;
}

const RECENT_ACTIVITY_LIMIT = 5;

function pluralize(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

function activityColorFromSeverity(severity: SiteEvent["severity"]): ActivityColor {
  if (severity === "critical") return "red";
  if (severity === "warning") return "amber";
  return "default";
}

function buildEventItem(
  event: ReturnType<typeof normalizeActivityFeedEvents>[number],
): DashboardActivityItem {
  return {
    id: event.id,
    label: event.title,
    value: event.summary || event.title,
    valueColor: activityColorFromSeverity(event.severity),
    eventType: event.eventType,
    source: event.source,
    occurredAt: new Date(event.occurredAtMs).toISOString(),
    target: "events",
    parsedDetail: event.parsedDetail,
  };
}

export function buildDashboardActivityFromEvents(events: SiteEvent[]): DashboardActivityItem[] {
  return normalizeActivityFeedEvents(events, { limit: RECENT_ACTIVITY_LIMIT }).map(buildEventItem);
}

function buildWebScanItem(latestWebScan: ScanResult, webIssueCount: number): DashboardActivityItem {
  return {
    id: "activity-web-scan",
    label: SCAN_LABELS.web,
    value: `${pluralize(webIssueCount, "issue")} found`,
    valueColor: webIssueCount > 0 ? "amber" : "green",
    eventType: "scan",
    source: SCAN_LABELS.web,
    occurredAt: latestWebScan.timestamp,
    target: "issues",
  };
}

function buildCodeScanItem(latestCodeScan: CodeScanSummary): DashboardActivityItem {
  return {
    id: "activity-code-scan",
    label: SCAN_LABELS.code,
    value: `${pluralize(latestCodeScan.issueCount, "issue")} found`,
    valueColor: latestCodeScan.criticalCount > 0 ? "amber" : "default",
    eventType: "scan",
    source: SCAN_LABELS.code,
    occurredAt: latestCodeScan.checkedAt,
    target: "code-scan",
  };
}

function canMergeAsFullScan(webScan: ScanResult | null, codeScan: CodeScanSummary | null): boolean {
  if (!webScan || !codeScan) return false;
  const delta = Math.abs(toEpoch(webScan.timestamp) - toEpoch(codeScan.checkedAt));
  return Number.isFinite(delta) && delta <= FULL_SCAN_MERGE_WINDOW_MS;
}

function buildFullScanItem(
  latestWebScan: ScanResult,
  webIssueCount: number,
  latestCodeScan: CodeScanSummary,
): DashboardActivityItem {
  const occurredAt =
    toEpoch(latestCodeScan.checkedAt) >= toEpoch(latestWebScan.timestamp)
      ? latestCodeScan.checkedAt
      : latestWebScan.timestamp;
  const valueColor: ActivityColor =
    latestCodeScan.criticalCount > 0
      ? "amber"
      : webIssueCount > 0 || latestCodeScan.issueCount > 0
        ? "amber"
        : "green";

  return {
    id: "activity-full-scan",
    label: SCAN_LABELS.full,
    value: `${pluralize(webIssueCount, "web issue")} · ${pluralize(latestCodeScan.issueCount, "code issue")}`,
    valueColor,
    eventType: "scan",
    source: SCAN_LABELS.full,
    occurredAt,
    target: "issues",
  };
}

function buildUpdateCheckItem(
  updatesCheckedAt: string,
  breakdown: UpdateQueueBreakdown,
): DashboardActivityItem {
  const totalUpdates = breakdown.critical + breakdown.major + breakdown.minor + breakdown.patch;
  const valueColor: ActivityColor =
    breakdown.critical > 0 ? "red" : totalUpdates > 0 ? "amber" : "green";

  return {
    id: "activity-update-check",
    label: "Update Check",
    value: formatUpdateBreakdown(breakdown),
    valueColor,
    eventType: "update",
    source: "Updates",
    occurredAt: updatesCheckedAt,
    target: "updates",
  };
}

export function buildDashboardActivity(inputs: DashboardActivityInputs): DashboardActivityItem[] {
  const items: DashboardActivityItem[] = [];

  if (inputs.latestDeploy) {
    items.push({
      id: "activity-deploy",
      label: "Deploy",
      value: `${inputs.latestDeploy.name} ${
        inputs.latestDeploy.conclusion === "failure"
          ? "failed"
          : inputs.latestDeploy.conclusion === "success"
            ? "passed"
            : inputs.latestDeploy.status
      }`,
      valueColor:
        inputs.latestDeploy.conclusion === "failure"
          ? "red"
          : inputs.latestDeploy.conclusion === "success"
            ? "green"
            : "default",
      eventType: "deploy",
      source: "Deploy",
      occurredAt: inputs.latestDeploy.updatedAt,
      target: "deploys",
    });
  }

  if (inputs.commitsSinceLastScan.length > 0) {
    items.push({
      id: "activity-commits",
      label: "Commits",
      value: `${pluralize(inputs.commitsSinceLastScan.length, "commit")} since last scan`,
      valueColor: "default",
      eventType: "deploy",
      source: "Git",
      occurredAt: inputs.commitsSinceLastScan[0].timestamp,
      target: "deploys",
    });
  }

  if (canMergeAsFullScan(inputs.latestWebScan, inputs.latestCodeScan)) {
    items.push(
      buildFullScanItem(
        inputs.latestWebScan as ScanResult,
        inputs.webIssueCount,
        inputs.latestCodeScan as CodeScanSummary,
      ),
    );
  } else {
    if (inputs.latestWebScan) {
      items.push(buildWebScanItem(inputs.latestWebScan, inputs.webIssueCount));
    }
    if (inputs.latestCodeScan) {
      items.push(buildCodeScanItem(inputs.latestCodeScan));
    }
  }

  if (inputs.updatesCheckedAt) {
    items.push(buildUpdateCheckItem(inputs.updatesCheckedAt, inputs.updateBreakdown));
  }

  return items
    .sort((a, b) => toEpoch(b.occurredAt) - toEpoch(a.occurredAt))
    .slice(0, RECENT_ACTIVITY_LIMIT);
}
