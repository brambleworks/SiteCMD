import { formatRelativeTime } from "@/lib/format";
import type { IssueGroup } from "@/lib/types";

const EVIDENCE_SOURCE_LABELS: Record<string, string> = {
  web_scan: "Web Scan",
  psi: "PSI",
  gsc: "GSC",
  code_scan: "Code Scan",
  updates: "Updates",
  uptime: "UptimeRobot",
};

export function labelForSource(src: string): string {
  return EVIDENCE_SOURCE_LABELS[src] ?? src;
}

export function formatRelativeDate(ts: number, nowMs: number): string {
  return formatRelativeTime(ts, nowMs, "verbose");
}

export function summarizeEvidence(group: IssueGroup, src: string, nowMs: number): string {
  const instances = group.instances.filter((i) => i.source === src);
  const first = instances[0];
  if (!first) return "";
  if (src === "web_scan") {
    return `Detected in scan since ${formatRelativeDate(first.firstSeenAt, nowMs)}`;
  }
  if (src === "psi") {
    return "Real users affected - see PSI report";
  }
  if (src === "gsc") {
    return `Google flagged this for ${instances.length} page${instances.length === 1 ? "" : "s"}`;
  }
  if (src === "code_scan") {
    const loc = first.signalId?.includes(":") ? first.signalId : (first.url ?? first.signalId);
    return `Found in code - ${loc}`;
  }
  const latest = instances.reduce((a, b) => (a.lastSeenAt > b.lastSeenAt ? a : b));
  return `Source observed ${formatRelativeDate(latest.lastSeenAt, nowMs)}`;
}
