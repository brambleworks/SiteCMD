import { command } from "./invoke";
import type {
  CodeScanReportPayload,
  ResolvedIssue,
  RunScanExecutionRequest,
  RunScanExecutionResult,
  ScanExecutionSummary,
  ScanExecutionDetail,
  ScanRunKind,
  ScanSchedule,
  ScheduledScanType,
  VerifyChecksResult,
} from "@/generated/ipc-bindings";

export function runScanExecution(args: {
  request: RunScanExecutionRequest;
}): Promise<RunScanExecutionResult> {
  return command<RunScanExecutionResult>("run_scan_execution", args);
}

export function cancelScan(args: { scanRequestId: number }): Promise<void> {
  return command<void>("cancel_scan", args);
}

export function verifyScanChecks(args: {
  projectId?: number | null;
  environmentUrl?: string | null;
  url: string;
  checkIds: string[];
  scanRequestId?: number | null;
  idempotencyKey?: string;
}): Promise<VerifyChecksResult> {
  return command<VerifyChecksResult>("verify_scan_checks", args);
}

export function getScanExecutions(args: {
  projectId?: number | null;
  environmentUrl?: string | null;
  runKind?: ScanRunKind | null;
  limit?: number;
}): Promise<ScanExecutionSummary[]> {
  return command<ScanExecutionSummary[]>("get_scan_executions", args);
}

export function getScanExecutionDetail(args: {
  executionId?: number | null;
  runId?: number | null;
}): Promise<ScanExecutionDetail | null> {
  return command<ScanExecutionDetail | null>("get_scan_execution_detail", args);
}

export function getResolvedIssues(args: {
  projectId: number;
  url: string;
  limit?: number;
}): Promise<ResolvedIssue[]> {
  return command<ResolvedIssue[]>("get_resolved_issues", args);
}

export function saveScanSchedule(args: {
  projectId: number;
  environmentId: number;
  frequency: string;
  timeOfDay: string;
  dayOfWeek?: number | null;
  scanType?: ScheduledScanType;
}): Promise<ScanSchedule> {
  return command<ScanSchedule>("save_scan_schedule", args);
}

export function getScanSchedule(args: {
  projectId: number;
  environmentId: number;
  scanType?: ScheduledScanType;
}): Promise<ScanSchedule | null> {
  return command<ScanSchedule | null>("get_scan_schedule", args);
}

export function runCodeScanAudit(args: {
  projectId: number;
  projectPath?: string | null;
  inspectLocalDatabases?: boolean;
}): Promise<CodeScanReportPayload> {
  return command<CodeScanReportPayload>("run_code_scan_audit", args);
}
