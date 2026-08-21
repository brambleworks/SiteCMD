import { normalizeHttpTargetUrl, type AppTarget } from "@/lib/app-targets";
import { buildUpdateBreakdownFromEventDetail, formatUpdateBreakdown } from "@/lib/activity-feed";
import { getScanArtifactLabel } from "@/lib/scan-labels";
import { formatUrlHostPath } from "@/lib/utils";

const HANDLED_DETAIL_KEYS = new Set([
  "url",
  "overall_score",
  "score",
  "issues_total",
  "critical_issues",
  "high_issues",
  "scan_type",
  "completed_pages",
  "duration_ms",
  "commit_sha",
  "branch",
  "status",
  "metric",
  "change_pct",
  "monitor",
  "downtime_minutes",
  "scan_id",
  "session_id",
  "project_id",
  "top_domain",
  "top_domain_count",
  "domain_trend_label",
  "page",
  "item_id",
  "lane",
  "reason",
  "verified_count",
  "rechecked_count",
  "cleared_count",
  "still_failing_count",
  "recurring_source_count",
  "recurring_source_cleared_count",
  "recurring_source_still_failing_count",
  "remaining_updates",
  "security_updates",
  "critical_updates",
  "major_updates",
  "minor_updates",
  "patch_updates",
  "item_label",
  "workflow_label",
  "passed_count",
  "remaining_blockers",
  "critical_before",
  "critical_after",
  "readiness_before",
  "readiness_after",
  "checked_count",
  "open_checks",
  "passed_checks",
  "focus",
  "focus_label",
  "priority_before",
  "priority_after",
  "code_issues_before",
  "code_issues_after",
  "verified_label",
  "next_item_label",
  "status_before",
  "status_after",
]);

export function humanizeEventDetail(detail: Record<string, unknown>, eventType: string): string[] {
  const pills: string[] = [];
  const compareEventTypes = ["search", "security", "update"];
  const nextLabelEventTypes = ["search", "security", "update", "verification"];
  const scanTypeLabel = detail.scan_type
    ? getScanArtifactLabel(String(detail.scan_type), { includeHealthSubtype: true })
    : null;
  const topDomainLabel = (() => {
    if (typeof detail.top_domain !== "string") return null;
    switch (detail.top_domain) {
      case "database":
        return "Database";
      case "ai-safety":
        return "AI Safety";
      case "security":
        return "Security";
      case "architecture":
        return "Architecture";
      case "operations":
        return "Operations";
      case "supply-chain":
        return "Dependencies";
      default:
        return String(detail.top_domain);
    }
  })();

  const updateBreakdown =
    eventType === "update" ? buildUpdateBreakdownFromEventDetail(detail) : null;

  if (updateBreakdown) {
    pills.push(formatUpdateBreakdown(updateBreakdown));
  }

  if (
    eventType !== "verification" &&
    eventType !== "update" &&
    detail.url &&
    typeof detail.url === "string"
  ) {
    pills.push(formatUrlHostPath(detail.url));
  }

  const score = scoreNumber(detail.overall_score ?? detail.score);
  if (score != null) pills.push(`Score: ${score}`);
  const issuesTotal = countNumber(detail.issues_total);
  if (issuesTotal != null) pills.push(`${issuesTotal} issues`);
  const criticalIssues = countNumber(detail.critical_issues);
  if (criticalIssues != null) pills.push(`${criticalIssues} critical`);
  const highIssues = countNumber(detail.high_issues);
  if (highIssues != null) pills.push(`${highIssues} high`);
  if (scanTypeLabel) pills.push(scanTypeLabel);
  if (topDomainLabel) {
    const domainCount = countNumber(detail.top_domain_count);
    const count = domainCount != null ? ` ${domainCount}` : "";
    pills.push(`${topDomainLabel}${count}`);
  }
  if (typeof detail.domain_trend_label === "string" && detail.domain_trend_label.trim()) {
    pills.push(detail.domain_trend_label);
  }
  if (typeof detail.workflow_label === "string" && detail.workflow_label.trim()) {
    pills.push(detail.workflow_label);
  }
  if (
    typeof detail.status_before === "string" &&
    typeof detail.status_after === "string" &&
    compareEventTypes.includes(eventType)
  ) {
    pills.push(`${detail.status_before} -> ${detail.status_after}`);
  }
  if (
    typeof detail.next_item_label === "string" &&
    detail.next_item_label.trim() &&
    nextLabelEventTypes.includes(eventType)
  ) {
    pills.push(`Next up: ${detail.next_item_label}`);
  }
  const recheckedCount = countNumber(detail.rechecked_count);
  if (recheckedCount != null) pills.push(`${recheckedCount} checked again`);
  const clearedCount = countNumber(detail.cleared_count);
  if (clearedCount != null) pills.push(`${clearedCount} cleared`);
  const stillFailingCount = countNumber(detail.still_failing_count);
  if (stillFailingCount != null && stillFailingCount > 0)
    pills.push(`${stillFailingCount} still open`);
  const recurringStillFailingCount = countNumber(detail.recurring_source_still_failing_count);
  const recurringClearedCount = countNumber(detail.recurring_source_cleared_count);
  if (recurringStillFailingCount != null && recurringStillFailingCount > 0) {
    pills.push(`Recurring code ${recurringStillFailingCount} still open`);
  } else if (recurringClearedCount != null && recurringClearedCount > 0) {
    pills.push(`Recurring code ${recurringClearedCount} cleared`);
  }
  const verifiedCount = countNumber(detail.verified_count);
  if (verifiedCount != null) pills.push(`${verifiedCount} verified`);
  const remainingUpdates = countNumber(detail.remaining_updates);
  if (remainingUpdates != null) pills.push(`${remainingUpdates} left`);
  const securityUpdates = countNumber(detail.security_updates);
  if (securityUpdates != null && securityUpdates > 0) pills.push(`${securityUpdates} security`);
  if (
    typeof detail.item_label === "string" &&
    eventType === "update" &&
    typeof detail.verified_label !== "string"
  )
    pills.push(detail.item_label);
  const checkedCount = countNumber(detail.checked_count);
  if (checkedCount != null) pills.push(`${checkedCount} checked`);
  const openChecks = countNumber(detail.open_checks);
  if (openChecks != null) pills.push(`${openChecks} still open`);
  const passedChecks = countNumber(detail.passed_checks);
  if (passedChecks != null) pills.push(`${passedChecks} passed`);
  if (typeof detail.focus_label === "string" && eventType === "search")
    pills.push(detail.focus_label);
  if (
    typeof detail.item_label === "string" &&
    eventType === "search" &&
    typeof detail.verified_label !== "string"
  )
    pills.push(detail.item_label);
  const passedCount = countNumber(detail.passed_count);
  if (passedCount != null) pills.push(`${passedCount} passed`);
  const remainingBlockers = countNumber(detail.remaining_blockers);
  if (remainingBlockers != null) pills.push(`${remainingBlockers} left`);
  const criticalBefore = countNumber(detail.critical_before);
  const criticalAfter = countNumber(detail.critical_after);
  if (criticalBefore != null && criticalAfter != null)
    pills.push(`Critical ${criticalBefore} -> ${criticalAfter}`);
  const priorityBefore = finiteNumber(detail.priority_before);
  const priorityAfter = finiteNumber(detail.priority_after);
  if (priorityBefore != null && priorityAfter != null)
    pills.push(`Priority ${priorityBefore} -> ${priorityAfter}`);
  const codeIssuesBefore = countNumber(detail.code_issues_before);
  const codeIssuesAfter = countNumber(detail.code_issues_after);
  if (codeIssuesBefore != null && codeIssuesAfter != null)
    pills.push(`Code issues ${codeIssuesBefore} -> ${codeIssuesAfter}`);
  if (
    typeof detail.item_label === "string" &&
    eventType === "security" &&
    typeof detail.verified_label !== "string"
  )
    pills.push(detail.item_label);
  if (typeof detail.focus_label === "string" && eventType === "security")
    pills.push(detail.focus_label);
  const completedPages = countNumber(detail.completed_pages);
  if (completedPages != null) pills.push(`${completedPages} pages`);
  const durationMs = finiteNumber(detail.duration_ms);
  if (durationMs != null) pills.push(`${(durationMs / 1000).toFixed(1)}s`);
  if (detail.commit_sha && typeof detail.commit_sha === "string")
    pills.push(String(detail.commit_sha).slice(0, 7));
  if (detail.branch) pills.push(String(detail.branch));
  if (detail.status && eventType.includes("deploy")) pills.push(String(detail.status));
  if (detail.metric) pills.push(String(detail.metric));
  const changePct = finiteNumber(detail.change_pct);
  if (changePct != null) pills.push(`${changePct > 0 ? "+" : ""}${changePct}%`);
  if (detail.monitor) pills.push(String(detail.monitor));
  const downtimeMinutes = finiteNumber(detail.downtime_minutes);
  if (downtimeMinutes != null) pills.push(`${downtimeMinutes}m down`);

  for (const [k, v] of Object.entries(detail)) {
    if (HANDLED_DETAIL_KEYS.has(k) || v == null || v === "") continue;
    if (pills.length >= 5) break;
    pills.push(`${k}: ${String(v)}`);
  }

  return pills.slice(0, 5);
}

function finiteNumber(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value !== "string" || !value.trim()) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function countNumber(value: unknown): number | null {
  const parsed = finiteNumber(value);
  if (parsed == null || parsed < 0) return null;
  return Math.round(parsed);
}

function scoreNumber(value: unknown): number | null {
  const parsed = finiteNumber(value);
  if (parsed == null) return null;
  return Math.max(0, Math.min(100, Math.round(parsed)));
}

function parsePositiveInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 ? value : null;
}

export function getUnhandledEventDetailEntries(
  detail: Record<string, unknown>,
): Array<[string, unknown]> {
  return Object.entries(detail).filter(
    ([key, value]) => !HANDLED_DETAIL_KEYS.has(key) && value != null && value !== "",
  );
}

export function buildEventScanTarget(
  projectId: number,
  detail: Record<string, unknown> | null,
): AppTarget | null {
  if (!detail) return null;
  const codeScanId = parsePositiveInteger(detail.code_scan_id);
  const siteScanId = parsePositiveInteger(detail.scan_id);
  const sessionId = parsePositiveInteger(detail.session_id);
  const detailUrl = normalizeHttpTargetUrl(typeof detail.url === "string" ? detail.url : null);
  const detailPage = typeof detail.page === "string" ? detail.page : null;
  const detailItemId = typeof detail.item_id === "string" ? detail.item_id : null;
  const detailLane = detail.lane === "pending-verification" ? "pending-verification" : null;
  const detailReason = typeof detail.reason === "string" ? detail.reason : null;

  if (detailPage === "updates") {
    return {
      page: "updates",
      projectId,
      url: detailUrl,
      itemId: detailItemId,
      ...(detailLane ? { lane: detailLane } : {}),
      reason: detailReason,
    };
  }
  if (detailPage === "search-console") {
    return {
      page: "search-console",
      projectId,
      url: detailUrl,
      itemId: detailItemId,
      ...(detailLane ? { lane: detailLane } : {}),
      focus: typeof detail.focus === "string" ? detail.focus : null,
      reason: detailReason,
    };
  }
  if (detailPage === "issues") {
    return {
      page: "issues",
      projectId,
      url: detailUrl,
      itemId: detailItemId,
      ...(detailLane ? { lane: detailLane } : {}),
      focus: typeof detail.focus === "string" ? detail.focus : null,
      reason: detailReason,
    };
  }
  if (codeScanId != null) {
    return { page: "issues", projectId, url: detailUrl, scanId: codeScanId, scanKind: "code" };
  }
  if (siteScanId != null) {
    return { page: "issues", projectId, url: detailUrl, scanId: siteScanId, scanKind: "site" };
  }
  if (sessionId != null) {
    return { page: "issues", projectId, url: detailUrl, sessionId };
  }
  return null;
}

export function getEventOpenLabel(target: AppTarget): string {
  if (target.page === "updates") {
    return target.itemId ? "Open Package Update" : "Open Updates";
  }
  if (target.page === "search-console") return "Open Search & SEO";
  if (target.scanKind === "code") return "View Code Scan";
  if (target.sessionId != null) return "View Multi-Page Scan";
  return "View Scan";
}
