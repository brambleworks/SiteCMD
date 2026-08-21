import { describe, expect, it } from "vitest";
import type { ConnectedAlert, ConnectedAlertCause } from "@/generated/ipc-bindings-connected";
import {
  alertSeverityLabel,
  alertTitle,
  causeLabel,
  deliverySummary,
  leadCause,
  outcomeLabel,
  outcomeToneClass,
  shouldRenderConnected,
  unavailableNotice,
} from "./connected-alert-display";

function cause(kind: string, severity: string | null, count = 1): ConnectedAlertCause {
  return { count, kind, severity };
}

function alert(overrides: Partial<ConnectedAlert> = {}): ConnectedAlert {
  return {
    alertId: "alr_1",
    causes: [cause("new_group", "high")],
    contentMode: "private",
    delivery: [],
    deploymentId: null,
    raisedAt: "2026-08-10T12:00:00.000Z",
    sequence: 1,
    severity: "high",
    updatedAt: null,
    ...overrides,
  };
}

describe("naming a connected alert", () => {
  it("leads with the worst severity, and a regression over an equal new finding", () => {
    // Keep row ordering consistent with connected alert email subjects.
    const lead = leadCause([
      cause("new_group", "critical"),
      cause("regression", "critical"),
      cause("new_group", "low"),
    ]);
    expect(lead?.kind).toBe("regression");
  });

  it("ranks a severity-bearing cause above one that bears none", () => {
    const lead = leadCause([cause("protection_degradation", null), cause("new_group", "low")]);
    expect(lead?.kind).toBe("new_group");
  });

  it("counts what the lead cause found and how many other classes there were", () => {
    expect(alertTitle(alert({ causes: [cause("regression", "critical", 3)] }))).toBe(
      "Regression of a verified fix (3)",
    );
    expect(
      alertTitle(
        alert({ causes: [cause("regression", "critical"), cause("new_group", "medium")] }),
      ),
    ).toBe("Regression of a verified fix and 1 more");
  });

  it("still names an alert the service minted with no cause lines", () => {
    expect(alertTitle(alert({ causes: [] }))).toBe("Alert raised");
    expect(leadCause([])).toBeNull();
  });

  it("keeps an unknown cause class visible instead of hiding it", () => {
    // A class this build has never heard of is still something the service
    // woke someone for.
    expect(causeLabel("some_future_class")).toBe("some future class");
  });

  it("says no severity rather than inventing one for the classes that bear none", () => {
    expect(alertSeverityLabel(null)).toBe("No severity");
    expect(alertSeverityLabel("critical")).toBe("Critical");
  });
});

describe("what the delivery record says", () => {
  it("states that nothing was sent rather than showing an empty line", () => {
    expect(deliverySummary([])).toBe("Sent to nobody: this site has no alert destination");
  });

  it("counts arrivals, failures, and everything else separately", () => {
    const summary = deliverySummary([
      { outcome: "sent", targetId: "dst_1", targetKind: "destination" },
      { outcome: "bounced", targetId: "dst_2", targetKind: "destination" },
      { outcome: "suppressed", targetId: "dst_3", targetKind: "destination" },
    ]);
    expect(summary).toBe("1 delivered, 1 did not arrive, 1 not sent");
  });

  it("reads only bounced and failed as a problem", () => {
    // Suppressed and not-sent are states the account chose; queued is in
    // flight. Painting any of them as failures would cry wolf.
    expect(outcomeToneClass("sent")).toBe("status-dot-success");
    expect(outcomeToneClass("bounced")).toBe("status-dot-critical");
    expect(outcomeToneClass("failed")).toBe("status-dot-critical");
    expect(outcomeToneClass("suppressed")).toBe("status-dot-warning");
    expect(outcomeToneClass("queued")).toBe("status-dot-muted");
  });

  it("says the service's outcome vocabulary plainly", () => {
    expect(outcomeLabel("not_sent")).toBe("Not sent");
    expect(outcomeLabel("indeterminate")).toBe("Unconfirmed");
  });
});

describe("when the connected timeline belongs on the page", () => {
  it("renders for a service that answered, and for one that refused to", () => {
    expect(shouldRenderConnected("ready")).toBe(true);
    expect(shouldRenderConnected("no_installation_token")).toBe(true);
    expect(shouldRenderConnected("not_entitled")).toBe(true);
  });

  it("stays off the page where there is no account to have read from", () => {
    // Not a degraded state: a project nobody connected has no connected
    // alerts to be missing, and a card explaining that would be noise.
    expect(shouldRenderConnected("site_not_connected")).toBe(false);
    expect(shouldRenderConnected("service_unconfigured")).toBe(false);
    expect(unavailableNotice("site_not_connected")).toBeNull();
    expect(unavailableNotice("ready")).toBeNull();
  });

  it("says which of the two unreadable states it is", () => {
    expect(unavailableNotice("no_installation_token")?.headline).toBe(
      "This machine cannot read the service",
    );
    expect(unavailableNotice("not_entitled")?.headline).toBe(
      "The connected service is not watching",
    );
  });
});
