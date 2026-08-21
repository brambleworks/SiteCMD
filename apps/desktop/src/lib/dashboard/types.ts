// src/lib/dashboard/types.ts
/** Top-level site verdict for Zone 1 */
type SiteVerdictKind = "healthy" | "attention" | "blocked";

export interface SiteVerdict {
  kind: SiteVerdictKind;
  /** One-line phrase, e.g., "Attention needed" */
  phrase: string;
  /** Short reasons (0-3 items) - first reason is primary driver */
  reasons: string[];
}

/** Inputs used to derive the verdict and critical rollup. */
export interface DashboardSnapshotInputs {
  criticalWebIssues: number;
  criticalCodeIssues: number;
  securityPatchCount: number;
  highWebIssues: number;
  deployFailed: boolean;
  integrationFailureCount: number;
  staleIntegrationCount: number;
  searchRegressionNegative: boolean;
  sslDaysRemaining: number | null;
}

/** Zone 2 Critical tile aggregation */
export interface CriticalRollup {
  total: number;
  web: number;
  code: number;
  securityPatches: number;
}

/** Zone 5b bootstrap task - one-time setup prompt */
export type BootstrapTaskKind =
  | "code-scan-link"
  | "code-scan-run"
  | "schedule"
  | "analytics"
  | "uptime"
  | "search"
  | "github"
  | "report"
  | "mcp";

export interface BootstrapTask {
  kind: BootstrapTaskKind;
  /** Source label displayed in the left column of the row */
  label: string;
  /** Value text displayed in the middle column */
  value: string;
  /** Navigation target; either a NavPage string or a pseudo-target the caller interprets */
  target: BootstrapTaskTarget;
  /** Ordering priority (lower number means shown first) */
  priority: number;
}

export type BootstrapTaskTarget =
  | { type: "nav"; page: string }
  | { type: "nav-settings"; tab: string }
  | { type: "action"; action: "add-folder" | "open-code-scan-config" };

/** Zone 5b setup-card row - a bootstrap task wired to its open action */
export interface SetupRow {
  id: string;
  label: string;
  value: string;
  onOpen: () => void;
}

/** Zone 5a activity row */
export interface ActivityRow {
  id: string;
  label: string;
  value: string;
  valueColor?: "default" | "amber" | "red" | "green";
  eventType?: string;
  source?: string;
  occurredAt: string;
  timeAgo: string;
  onOpen: () => void;
}

/** Zone 1 stack chip data */
/** SSL probe result returned by the Rust backend */
export interface SslProbeResult {
  days_remaining: number | null;
  auto_renew_hint: boolean;
  not_after_iso: string | null;
  /** Error message if the probe failed; result fields are null when set */
  error: string | null;
}
