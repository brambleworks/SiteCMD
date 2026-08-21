import { formatRelativeTime } from "@/lib/format";
import type { AlertRow } from "@/lib/types";

type AlertSourceKey = "uptimerobot" | "cloudflare" | "plausible" | "ga4" | "gsc" | "github";

export interface NativeAlertDefinition {
  id: string;
  label: string;
  trigger: string;
  detail: string;
  cadence: string;
}

export interface AlertSourceDefinition {
  source: AlertSourceKey;
  integrationType: string;
  label: string;
  trigger: string;
  detail: string;
  cadence: string;
}

export const NATIVE_ALERT_DEFINITIONS: NativeAlertDefinition[] = [
  {
    id: "web-regressions",
    label: "Web Scans",
    trigger: "Diagnostic drops and new criticals",
    detail:
      "Alerts when the latest Web Scan diagnostics meaningfully regress or critical issues appear.",
    cadence: "After each scan",
  },
  {
    id: "code-regressions",
    label: "Code Scans",
    trigger: "New critical findings",
    detail:
      "Alerts when the latest Code Scan finds new critical code issues. Routine diagnostic movement stays in Issues.",
    cadence: "After each scan",
  },
  {
    id: "scan-failures",
    label: "Scan Health",
    trigger: "Failed scans",
    detail: "Alerts when SiteCMD could not complete a scan, before it can become an Issue.",
    cadence: "As it happens",
  },
  {
    id: "dependency-updates",
    label: "Dependency Updates",
    trigger: "Vulnerabilities and SSL expiry",
    detail: "Alerts when package security risk or expiring SSL needs fast attention.",
    cadence: "Hourly + manual check",
  },
];

export const ALERT_SOURCE_DEFINITIONS: AlertSourceDefinition[] = [
  {
    source: "uptimerobot",
    integrationType: "uptimerobot",
    label: "UptimeRobot",
    trigger: "Downtime",
    detail: "Creates a critical alert when a connected monitor reports the site is down.",
    cadence: "Every 60s",
  },
  {
    source: "cloudflare",
    integrationType: "cloudflare",
    label: "Cloudflare",
    trigger: "Threat traffic blocked",
    detail:
      "Creates a warning or critical alert when Cloudflare reports blocked requests classified as threats.",
    cadence: "Every 5m",
  },
  {
    source: "plausible",
    integrationType: "plausible",
    label: "Plausible",
    trigger: "Traffic anomalies",
    detail: "Creates an alert for meaningful traffic spikes or drops once traffic is high enough.",
    cadence: "Every 5m",
  },
  {
    source: "ga4",
    integrationType: "googleanalytics",
    label: "Google Analytics",
    trigger: "Traffic anomalies",
    detail:
      "Creates an alert for meaningful GA4 traffic spikes or drops once traffic is high enough.",
    cadence: "Every 5m",
  },
  {
    source: "gsc",
    integrationType: "googlesearchconsole",
    label: "Search Console",
    trigger: "Search impression drops",
    detail: "Creates an alert when a query loses enough impressions to warrant attention.",
    cadence: "Hourly",
  },
  {
    source: "github",
    integrationType: "github",
    label: "GitHub",
    trigger: "CI failures",
    detail: "Creates an alert when the linked repository reports a failed GitHub Actions run.",
    cadence: "Hourly + manual check",
  },
];

export function labelForSource(source: string): string {
  if (source === "sitecmd") return "SiteCMD";
  if (source === "updates") return "Dependency Updates";
  return (
    ALERT_SOURCE_DEFINITIONS.find((definition) => definition.source === source)?.label ?? source
  );
}

export function severityLabel(severity: AlertRow["severity"]): string {
  if (severity === "critical") return "Critical";
  if (severity === "warn") return "Warning";
  return "Info";
}

export function severityToneClass(severity: AlertRow["severity"]): string {
  if (severity === "critical") return "text-severity-critical";
  if (severity === "warn") return "text-severity-medium";
  return "text-severity-low";
}

export function formatRelative(timestamp: number, nowMs: number): string {
  return formatRelativeTime(timestamp, nowMs);
}

export function formatAbsolute(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return "Not recorded";
  return new Date(timestamp).toLocaleString();
}
