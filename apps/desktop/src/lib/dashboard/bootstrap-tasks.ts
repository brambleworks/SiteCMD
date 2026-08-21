import type { BootstrapTask, BootstrapTaskKind, BootstrapTaskTarget } from "./types";

export interface BootstrapInputs {
  hasProjectFolder: boolean;
  hasCodeScan: boolean;
  hasSchedule: boolean;
  hasAnalytics: boolean;
  hasUptime: boolean;
  hasSearch: boolean;
  hasGithub: boolean;
  hasReportSchedule: boolean;
  mcpConfigured: boolean;
}

interface Candidate {
  kind: BootstrapTaskKind;
  applies: (i: BootstrapInputs) => boolean;
  label: string;
  value: string;
  target: BootstrapTaskTarget;
  priority: number;
}

const CANDIDATES: Candidate[] = [
  {
    kind: "code-scan-run",
    applies: (i) => i.hasProjectFolder && !i.hasCodeScan,
    label: "Code scan",
    value: "Run your first code scan",
    target: { type: "action", action: "open-code-scan-config" },
    priority: 10,
  },
  {
    kind: "code-scan-link",
    applies: (i) => !i.hasProjectFolder && !i.hasCodeScan,
    label: "Code scan",
    value: "Link project folder to audit code",
    target: { type: "action", action: "add-folder" },
    priority: 10,
  },
  {
    kind: "schedule",
    applies: (i) => !i.hasSchedule,
    label: "Schedule",
    value: "Schedule recurring scans",
    // The recurring-scan scheduler lives in Settings > Scanning, not the
    // run-now scan form. Route there so the link opens the actual scheduler.
    target: { type: "nav-settings", tab: "scanning" },
    priority: 20,
  },
  {
    kind: "analytics",
    applies: (i) => !i.hasAnalytics,
    label: "Analytics",
    value: "Connect traffic source (Plausible, GA4, Cloudflare)",
    target: { type: "nav-settings", tab: "integrations" },
    priority: 30,
  },
  {
    kind: "uptime",
    applies: (i) => !i.hasUptime,
    label: "Uptime",
    value: "Add UptimeRobot to monitor availability",
    target: { type: "nav-settings", tab: "integrations" },
    priority: 40,
  },
  {
    kind: "search",
    applies: (i) => !i.hasSearch,
    label: "Search",
    value: "Connect Google Search Console or Bing Webmaster",
    target: { type: "nav-settings", tab: "integrations" },
    priority: 50,
  },
  {
    kind: "github",
    applies: (i) => !i.hasGithub,
    label: "GitHub",
    value: "Connect GitHub for deploys and PRs",
    target: { type: "nav-settings", tab: "integrations" },
    priority: 60,
  },
  {
    kind: "report",
    applies: (i) => !i.hasReportSchedule,
    label: "Report",
    value: "Configure a recurring report",
    target: { type: "nav", page: "reports" },
    priority: 80,
  },
  {
    kind: "mcp",
    applies: (i) => !i.mcpConfigured,
    label: "MCP",
    value: "Wire MCP server into Cursor or Claude Code",
    target: { type: "nav-settings", tab: "integrations" },
    priority: 90,
  },
];

export function buildBootstrapTasks(input: BootstrapInputs): BootstrapTask[] {
  return CANDIDATES.filter((c) => c.applies(input)).map((c) => ({
    kind: c.kind,
    label: c.label,
    value: c.value,
    target: c.target,
    priority: c.priority,
  }));
}
