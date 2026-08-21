import { isJsonRecord } from "@/lib/json-record";
import { errorMessage } from "@/lib/error-message";
import { trackDiagnosticEvent, trackUsageEvent } from "@/lib/telemetry";

type WorkflowHealthName =
  "add_site" | "run_scan" | "open_issues" | "copy_guidance" | "verify_issue";

type WorkflowHealthStatus = "started" | "succeeded" | "failed";

type ErrorSource =
  | "window.error"
  | "window.unhandledrejection"
  | "react.error_boundary"
  | "startup.watchdog"
  | "startup.bootstrap";

type PrimitiveMetaValue = string | number | boolean | null;

interface WorkflowHealthEvent {
  kind: "workflow";
  name: WorkflowHealthName;
  status: WorkflowHealthStatus;
  timestamp: string;
  meta?: Record<string, PrimitiveMetaValue>;
}

interface ErrorReportEvent {
  kind: "error";
  source: ErrorSource;
  fatal: boolean;
  timestamp: string;
  message: string;
  meta?: Record<string, PrimitiveMetaValue>;
}

interface ObservabilityStore {
  workflow: WorkflowHealthEvent[];
  errors: ErrorReportEvent[];
}

const STORAGE_KEY = "sitecmd_observability_v1";
const MAX_WORKFLOW_EVENTS = 250;
const MAX_ERROR_EVENTS = 120;
const WORKFLOW_NAMES: WorkflowHealthName[] = [
  "add_site",
  "run_scan",
  "open_issues",
  "copy_guidance",
  "verify_issue",
];
const ERROR_SOURCES: ErrorSource[] = [
  "window.error",
  "window.unhandledrejection",
  "react.error_boundary",
  "startup.watchdog",
  "startup.bootstrap",
];

function readStore(): ObservabilityStore {
  if (typeof window === "undefined" || !window.localStorage) {
    return { workflow: [], errors: [] };
  }

  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return { workflow: [], errors: [] };
    return parseStore(JSON.parse(raw) as unknown);
  } catch {
    return { workflow: [], errors: [] };
  }
}

function parseStore(value: unknown): ObservabilityStore {
  if (!isJsonRecord(value)) return { workflow: [], errors: [] };
  return {
    workflow: Array.isArray(value.workflow)
      ? value.workflow
          .flatMap((event) => parseWorkflowHealthEvent(event) ?? [])
          .slice(-MAX_WORKFLOW_EVENTS)
      : [],
    errors: Array.isArray(value.errors)
      ? value.errors.flatMap((event) => parseErrorReportEvent(event) ?? []).slice(-MAX_ERROR_EVENTS)
      : [],
  };
}

function parseWorkflowHealthEvent(value: unknown): WorkflowHealthEvent | null {
  if (
    !isJsonRecord(value) ||
    value.kind !== "workflow" ||
    !WORKFLOW_NAMES.includes(value.name as WorkflowHealthName) ||
    (value.status !== "started" && value.status !== "succeeded" && value.status !== "failed") ||
    typeof value.timestamp !== "string"
  ) {
    return null;
  }

  return {
    kind: "workflow",
    name: value.name as WorkflowHealthName,
    status: value.status,
    timestamp: value.timestamp,
    meta: sanitizePersistedMeta(value.meta),
  };
}

function parseErrorReportEvent(value: unknown): ErrorReportEvent | null {
  if (
    !isJsonRecord(value) ||
    value.kind !== "error" ||
    !ERROR_SOURCES.includes(value.source as ErrorSource) ||
    typeof value.fatal !== "boolean" ||
    typeof value.timestamp !== "string" ||
    typeof value.message !== "string"
  ) {
    return null;
  }

  return {
    kind: "error",
    source: value.source as ErrorSource,
    fatal: value.fatal,
    timestamp: value.timestamp,
    message: sanitizeText(value.message),
    meta: sanitizePersistedMeta(value.meta),
  };
}

function writeStore(store: ObservabilityStore) {
  if (typeof window === "undefined" || !window.localStorage) return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
  } catch {
    // Best-effort only.
  }
}

function sanitizeText(value: string): string {
  return value
    .replace(/https?:\/\/\S+/gi, "[url]")
    .replace(/\b(?:localhost|127\.0\.0\.1)(?::\d+)?\b/gi, "[local-url]")
    .replace(/\/(?:Users|home|var|tmp|private|Volumes)\/[^\s)]+/g, "[path]")
    .replace(/[A-Z]:\\[^\s)]+/g, "[path]")
    .replace(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi, "[email]")
    .replace(/\b(ghp|sk|rk|pk)_[A-Za-z0-9_-]+\b/g, "[secret]")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 240);
}

function sanitizeMetaValue(value: unknown): PrimitiveMetaValue | undefined {
  if (value == null) return null;
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return Number.isFinite(value) ? value : undefined;
  if (typeof value === "string") {
    const sanitized = sanitizeText(value);
    return sanitized.length > 0 ? sanitized : null;
  }
  return undefined;
}

function sanitizeMeta(
  meta?: Record<string, unknown>,
): Record<string, PrimitiveMetaValue> | undefined {
  if (!meta) return undefined;
  const entries = Object.entries(meta)
    .map(([key, value]) => [key, sanitizeMetaValue(value)] as const)
    .filter((entry): entry is [string, PrimitiveMetaValue] => entry[1] !== undefined)
    .slice(0, 12);
  if (entries.length === 0) return undefined;
  return Object.fromEntries(entries);
}

function sanitizePersistedMeta(value: unknown): Record<string, PrimitiveMetaValue> | undefined {
  return isJsonRecord(value) ? sanitizeMeta(value) : undefined;
}

function appendWorkflowEvent(event: WorkflowHealthEvent) {
  const store = readStore();
  store.workflow.push(event);
  if (store.workflow.length > MAX_WORKFLOW_EVENTS) {
    store.workflow = store.workflow.slice(-MAX_WORKFLOW_EVENTS);
  }
  writeStore(store);
}

function appendErrorEvent(event: ErrorReportEvent) {
  const store = readStore();
  store.errors.push(event);
  if (store.errors.length > MAX_ERROR_EVENTS) {
    store.errors = store.errors.slice(-MAX_ERROR_EVENTS);
  }
  writeStore(store);
}

export function recordWorkflowHealthEvent(
  name: WorkflowHealthName,
  status: WorkflowHealthStatus,
  meta?: Record<string, unknown>,
) {
  const sanitizedMeta = sanitizeMeta(meta);
  appendWorkflowEvent({
    kind: "workflow",
    name,
    status,
    timestamp: new Date().toISOString(),
    meta: sanitizedMeta,
  });
  trackUsageEvent("workflow_event", {
    workflowName: name,
    workflowStatus: status,
    ...(sanitizedMeta ?? {}),
  });
}

export function recordErrorReport(
  source: ErrorSource,
  error: unknown,
  options?: {
    fatal?: boolean;
    message?: string;
    meta?: Record<string, unknown>;
  },
) {
  const derivedMessage = options?.message ?? errorMessage(error);

  appendErrorEvent({
    kind: "error",
    source,
    fatal: options?.fatal ?? false,
    timestamp: new Date().toISOString(),
    message: sanitizeText(derivedMessage || "Unknown error"),
    meta: sanitizeMeta(options?.meta),
  });
  trackDiagnosticEvent(
    source === "startup.bootstrap" || source === "startup.watchdog"
      ? "startup_error"
      : "frontend_error",
    error,
    {
      source,
      fatal: options?.fatal ?? false,
      ...(options?.meta ?? {}),
    },
  );
}

export function clearObservabilitySnapshot() {
  if (typeof window === "undefined" || !window.localStorage) return;
  window.localStorage.removeItem(STORAGE_KEY);
}

function formatMeta(meta?: Record<string, PrimitiveMetaValue>): string {
  if (!meta) return "";
  const parts = Object.entries(meta).map(([key, value]) => `${key}=${String(value)}`);
  return parts.length > 0 ? ` (${parts.join(", ")})` : "";
}

function summarizeWorkflowHealth(events: WorkflowHealthEvent[]): string[] {
  const lines: string[] = [];

  for (const name of WORKFLOW_NAMES) {
    const matching = events.filter((event) => event.name === name);
    const started = matching.filter((event) => event.status === "started").length;
    const succeeded = matching.filter((event) => event.status === "succeeded").length;
    const failed = matching.filter((event) => event.status === "failed").length;
    const label = name.replace(/_/g, " ");
    lines.push(`- ${label}: ${succeeded} succeeded, ${failed} failed, ${started} started`);
  }

  return lines;
}

function summarizeHealthSignals(
  events: WorkflowHealthEvent[],
  errors: ErrorReportEvent[],
): string[] {
  const lines: string[] = [];

  const addSiteFailures = events.filter(
    (event) => event.name === "add_site" && event.status === "failed",
  ).length;
  const scanFailures = events.filter(
    (event) => event.name === "run_scan" && event.status === "failed",
  ).length;
  const issuesFailures = events.filter(
    (event) => event.name === "open_issues" && event.status === "failed",
  ).length;
  const fatalErrors = errors.filter((event) => event.fatal).length;

  lines.push(
    addSiteFailures > 0
      ? `- onboarding: needs attention (${addSiteFailures} failed add-site attempt${addSiteFailures === 1 ? "" : "s"})`
      : "- onboarding: no recent add-site failures recorded",
  );
  lines.push(
    scanFailures > 0
      ? `- scans: needs attention (${scanFailures} failed scan run${scanFailures === 1 ? "" : "s"})`
      : "- scans: no recent scan failures recorded",
  );
  lines.push(
    issuesFailures > 0
      ? `- issues: needs attention (${issuesFailures} failed Issues load${issuesFailures === 1 ? "" : "s"})`
      : "- issues: no recent Issues-load failures recorded",
  );
  lines.push(
    fatalErrors > 0
      ? `- crashes: needs attention (${fatalErrors} fatal crash/error event${fatalErrors === 1 ? "" : "s"})`
      : "- crashes: no fatal crash events recorded",
  );

  return lines;
}

export function buildObservabilitySnapshotText(): string {
  const store = readStore();
  const workflow = store.workflow;
  const errors = store.errors;

  const lines = ["SiteCMD Observability Snapshot", `Updated: ${new Date().toISOString()}`, ""];

  if (workflow.length === 0 && errors.length === 0) {
    lines.push("No observability events captured yet.");
    return lines.join("\n");
  }

  lines.push("Health Signals");
  lines.push(...summarizeHealthSignals(workflow, errors));
  lines.push("");

  lines.push("Workflow Health");
  lines.push(...summarizeWorkflowHealth(workflow));
  lines.push("");

  lines.push(`Crash / error events recorded: ${errors.length}`);
  if (errors.length > 0) {
    lines.push("Recent crash / error events");
    for (const event of errors.slice(-10).reverse()) {
      lines.push(`- ${event.timestamp} ${event.source}: ${event.message}${formatMeta(event.meta)}`);
    }
    lines.push("");
  }

  const recentFailures = workflow
    .filter((event) => event.status === "failed")
    .slice(-10)
    .reverse();

  lines.push(`Recent workflow failures: ${recentFailures.length}`);
  if (recentFailures.length > 0) {
    for (const event of recentFailures) {
      lines.push(`- ${event.timestamp} ${event.name}${formatMeta(event.meta)}`);
    }
  }

  return lines.join("\n");
}
