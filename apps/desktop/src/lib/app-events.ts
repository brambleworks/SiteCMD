import { emit } from "@tauri-apps/api/event";

import type { ActionableDesktopNotificationEvent } from "@/lib/actionable-notifications";
import type { ProjectSignalsChangedEvent } from "@/lib/project-signal-events";
import type { CodeScanDomain, IssueStatus } from "@/lib/types";

// Canonical event names and payloads for the Tauri event bridge. React consumers
// subscribe through useTauriEvent; query-only reactions use event-invalidation.

interface LicenseActivatePayload {
  key: string;
}

interface CliImportEventPayload {
  project_id: number;
  name: string;
  url: string;
  imported_scan: boolean;
  scan_id?: number | null;
}

interface ScheduledScanCompletePayload {
  executionId?: number;
  projectId: number;
  url: string;
  scanId?: number | null;
  score: number;
  issues: number;
  scanType?: "health" | "security" | "code" | "full" | string;
  status: "complete" | "partial";
  completedPages?: number | null;
  totalPages?: number | null;
  incompleteDetail?: string | null;
  timestamp?: string;
  topDomain?: CodeScanDomain | null;
  topDomainCount?: number;
  domainTrendLabel?: string | null;
}

export interface AppEventPayloads {
  // License activation deep link.
  "sitecmd-license-activate-requested": LicenseActivatePayload;
  // Desktop notification action.
  "desktop-notification-action": ActionableDesktopNotificationEvent;
  // Fix-attempt state changed.
  "fix-attempt-updated": void;
  // A verified catalog pack became active.
  "catalog-updated": void;
  // A catalog refresh tick reached a terminal outcome.
  "catalog-refresh-completed": void;
  // Google integration connection changed.
  "google-integration-updated": { projectId?: number };
  // CLI project import deep link.
  "sitecmd-cli-imported": CliImportEventPayload;
  // Tray actions.
  "tray-open-overview": void;
  "tray-scan-now": void;
  "tray-show-scan": void;
  // Scheduled scan completed.
  "scheduled-scan-complete": ScheduledScanCompletePayload;
  // Admitted scan execution reached a terminal state.
  "scan-execution-completed": {
    executionId: number;
    projectId: number | null;
    requestedMode: "full" | "web" | "code";
    status: "complete" | "partial" | "failed" | "cancelled";
    webStatus: "planned" | "running" | "complete" | "failed" | "cancelled" | "skipped" | null;
    codeStatus: "planned" | "running" | "complete" | "failed" | "cancelled" | "skipped" | null;
    codeRunId: number | null;
  };
  // Project score or underlying work items changed.
  "site-score-changed": { projectId?: number };
  // Dashboard signals changed for a project scope.
  "project-signals-changed": ProjectSignalsChangedEvent;
  // One issue lifecycle transitioned.
  "issue-lifecycle-changed": { projectId: number; checkId: string; status: IssueStatus };
  // Alert rows or counts changed.
  "alerts-changed": { projectId: number | null };
  // Timeline rows were written outside a scan. Emit through event-writes.
  "events-recorded": { projectId: number };
  // An issue integration hint was dismissed.
  "integration-hint-dismissed": { projectId: number; checkId: string; integration: string };
}

/** Every registered Tauri app-event name. */
export type AppEventName = keyof AppEventPayloads;

/** Emit a typed, best-effort app event. */
export function emitAppEvent<K extends AppEventName>(event: K, payload: AppEventPayloads[K]): void {
  void emit(event, payload).catch(() => {
    // The next poll or event recovers.
  });
}
