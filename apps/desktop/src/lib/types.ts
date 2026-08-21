/** Generated Rust IPC types plus frontend-only refinements. */

export type {
  // scan engine
  ScanCategory,
  ScanType,
  ScheduledScanType,
  CheckResult,
  CategoryScore,
  ScanResult,
  RunScanExecutionRequest,
  RunScanExecutionResult,
  ScanExecutionSummary,
  ScanTrigger,
  // events
  EventSeverity,
  // updates
  Ecosystem,
  PackageUpdate,
  UpdateReport,
  // code scan
  CodeScanDomain,
  CodeScanSummary,
  CodeScanDomainSummary,
  CodeScanResult,
  // issue links
  IssueLink,
  // work items / unified issues
  PageSummary,
  LikelyCause,
  TransitiveCause,
  RecentEventRef,
  Enrichment,
  Evidence,
  CrossEnvSignal,
  CrossProjectPattern,
  IntegrationType,
  IntegrationSuggestion,
  FixLocation,
  IssueStatus,
  IssueGroup,
  // alerts
  AlertRow,
  AlertFilter,
  // score
  ScoreSnapshot,
  ScoreBreakdown,
} from "@/generated/ipc-bindings";

import type {
  CodeIssueView,
  CodeScanReportPayload,
  SiteEvent as GeneratedSiteEvent,
} from "@/generated/ipc-bindings";

export type { IssueConfidence } from "./issue-confidence";
export type { Severity } from "./severity";

// Frontend alias for the domain-tagged CodeIssueView wire type.
export type CodeIssue = CodeIssueView;

// The non-persisted code-audit report payload (`run_code_scan_audit`).
export type CodeScanReport = CodeScanReportPayload;

// SiteEvent carries a client-parsed `parsedDetail` derived from `detail`; the
// rest of the shape is generated from Rust.
export type SiteEvent = GeneratedSiteEvent & {
  parsedDetail?: Record<string, unknown> | null;
};

// Frontend-only unions with no Rust struct backing.
export type IssueScope = "page" | "site" | "code";

// Score-band helpers live in `./score` - the single source of truth.
// Re-exported here so existing `@/lib/types` import sites keep working.
export { getScoreClass, getScoreLabel, getScoreMessage } from "./score";
