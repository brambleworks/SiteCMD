import type {
  ConnectedAlert,
  ConnectedAlertCause,
  ConnectedAlertDelivery,
} from "@/generated/ipc-bindings-connected";
import {
  SEVERITIES,
  formatSeverityLabel,
  formatSeverityToneClass,
  isSeverity,
  severityRank,
} from "@/lib/severity";

// Unknown cause kinds remain visible through a generated fallback label.
const CAUSE_LABELS: Record<string, string> = {
  certificate_horizon: "Certificate expiry approaching",
  claim_not_confirmed: "Fix claim not confirmed",
  coverage_available: "New coverage available",
  detector_or_corpus_update: "Changed by a SiteCMD release",
  measurement_threshold_crossing: "Measurement crossed a threshold",
  new_group: "New finding",
  new_occurrence_in_active_group: "New occurrence in an open finding",
  protection_degradation: "Protection degraded",
  regression: "Regression of a verified fix",
  snooze_expiry: "Snooze ran out",
  verification_success: "Fix verified",
};

export function causeLabel(kind: string): string {
  return CAUSE_LABELS[kind] ?? kind.replace(/_/g, " ");
}

function causeRank(cause: ConnectedAlertCause): number {
  return isSeverity(cause.severity) ? severityRank(cause.severity) : SEVERITIES.length;
}

/** Returns the highest-severity cause, preferring regressions on ties. */
export function leadCause(causes: readonly ConnectedAlertCause[]): ConnectedAlertCause | null {
  if (causes.length === 0) return null;
  return [...causes].sort((left, right) => {
    const gap = causeRank(left) - causeRank(right);
    if (gap !== 0) return gap;
    if (left.kind === right.kind) return 0;
    if (left.kind === "regression") return -1;
    if (right.kind === "regression") return 1;
    return 0;
  })[0];
}

/** The row's one-line title: what happened, and how much of it. */
export function alertTitle(alert: ConnectedAlert): string {
  const lead = leadCause(alert.causes);
  if (!lead) return "Alert raised";
  const label = causeLabel(lead.kind);
  const counted = lead.count > 1 ? `${label} (${lead.count})` : label;
  const others = alert.causes.length - 1;
  return others > 0 ? `${counted} and ${others} more` : counted;
}

export function alertSeverityLabel(severity: string | null): string {
  return severity === null ? "No severity" : formatSeverityLabel(severity);
}

export function alertSeverityToneClass(severity: string | null): string {
  return severity === null ? "text-muted-foreground" : formatSeverityToneClass(severity);
}

const OUTCOME_LABELS: Record<string, string> = {
  bounced: "Bounced",
  failed: "Failed",
  indeterminate: "Unconfirmed",
  not_sent: "Not sent",
  queued: "Queued",
  sent: "Delivered",
  suppressed: "Suppressed",
};

export function outcomeLabel(outcome: string): string {
  return OUTCOME_LABELS[outcome] ?? outcome.replace(/_/g, " ");
}

/** Maps delivery outcomes to tones; only bounced and failed are critical. */
export function outcomeToneClass(outcome: string): string {
  if (outcome === "sent") return "status-dot-success";
  if (outcome === "bounced" || outcome === "failed") return "status-dot-critical";
  if (outcome === "queued") return "status-dot-muted";
  return "status-dot-warning";
}

/** An absolute stamp for the ISO instants the service reports. */
export function formatAlertTimestamp(value: string | null): string {
  if (!value) return "Not recorded";
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? new Date(parsed).toLocaleString() : value;
}

export function targetKindLabel(targetKind: string): string {
  if (targetKind === "destination") return "Email";
  if (targetKind === "webhook") return "Webhook";
  return targetKind;
}

export function deliverySummary(delivery: readonly ConnectedAlertDelivery[]): string {
  if (delivery.length === 0) return "Sent to nobody: this site has no alert destination";
  const delivered = delivery.filter((cell) => cell.outcome === "sent").length;
  const failed = delivery.filter(
    (cell) => cell.outcome === "bounced" || cell.outcome === "failed",
  ).length;
  const parts: string[] = [];
  if (delivered > 0) parts.push(`${delivered} delivered`);
  if (failed > 0) parts.push(`${failed} did not arrive`);
  const remaining = delivery.length - delivered - failed;
  if (remaining > 0) parts.push(`${remaining} not sent`);
  return parts.join(", ");
}

export interface ConnectedUnavailableNotice {
  headline: string;
  detail: string;
}

export function unavailableNotice(availability: string): ConnectedUnavailableNotice | null {
  if (availability === "no_installation_token") {
    return {
      detail:
        "This environment is connected, but this machine holds no installation token, so it cannot read what the service has raised. Activating the connected service in Settings gives it one.",
      headline: "This machine cannot read the service",
    };
  }
  if (availability === "not_entitled") {
    return {
      detail:
        "The subscription behind this account is not active, so the service has stopped watching its sites and will not answer for their alerts. Local scans are unaffected.",
      headline: "The connected service is not watching",
    };
  }
  // Unconfigured sites have no degraded connected state to report.
  return null;
}

/** Whether the connected timeline belongs on the page at all. */
export function shouldRenderConnected(availability: string): boolean {
  return availability === "ready" || unavailableNotice(availability) !== null;
}
