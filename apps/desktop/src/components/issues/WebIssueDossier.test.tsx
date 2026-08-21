import React from "react";
import { act, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CheckResult } from "@/lib/types";

const { invokeMock, agentActionProps, actionBarProps, verifyIssueMock, toastError, toastSuccess } =
  vi.hoisted(() => ({
    invokeMock: vi.fn(),
    agentActionProps: [] as Array<Record<string, unknown>>,
    actionBarProps: [] as Array<Record<string, unknown>>,
    verifyIssueMock: vi.fn(),
    toastError: vi.fn(),
    toastSuccess: vi.fn(),
  }));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: toastSuccess,
    warning: vi.fn(),
    error: toastError,
  }),
}));
vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    hasFeature: (feature: string) => feature === "issue_rich_detail",
    licenseInfo: { checkout_urls: null },
  }),
}));
vi.mock("@/components/ui/button", () => ({
  Button: ({
    children,
    type = "button",
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) =>
    React.createElement("button", { type, ...props }, children),
}));
vi.mock("@/components/issues/IssueDossierPanel", async () => {
  const { buildDossierPanelMock } = await import("@/test-utils/dossier-panel-mock");
  return buildDossierPanelMock();
});
vi.mock("@/components/issues/IssueActionBar", () => ({
  IssueActionBar: (props: Record<string, unknown> & { extraActions?: React.ReactNode }) => {
    actionBarProps.push(props);
    return React.createElement("div", null, "IssueActionBar", props.extraActions);
  },
}));
vi.mock("@/components/issues/FixWithAgentAction", () => ({
  FixWithAgentAction: (props: Record<string, unknown>) => {
    agentActionProps.push(props);
    return React.createElement("div", null, "FixWithAgentAction");
  },
}));
vi.mock("@/components/issues/SendToTrackerAction", () => ({
  SendToTrackerAction: () => React.createElement("div", null, "SendToTrackerAction"),
}));
vi.mock("@/components/issues/IssueMemorySection", () => ({
  IssueMemorySection: () => null,
  IssueMemoryRail: () => null,
}));
vi.mock("./WebIssueDossierBody", () => ({
  WebIssueRichSections: () => React.createElement("div", null, "WebIssueRichSections"),
}));
vi.mock("@/lib/issues", () => ({
  verifyIssue: verifyIssueMock,
}));
vi.mock("@/lib/desktop-actions", () => ({
  openPathInEditor: vi.fn(() => Promise.resolve()),
  revealPath: vi.fn(() => Promise.resolve()),
  runProjectCommand: vi.fn(() => Promise.resolve({ success: true, stdout: "", stderr: "" })),
  isProjectCommandCancelled: vi.fn(() => false),
}));
vi.mock("@/lib/issue-scope", () => ({
  getCheckIssueScope: vi.fn(() => ({ scopeLabel: "This page" })),
}));
vi.mock("@/lib/pending-verification", () => ({
  buildPendingVerificationId: vi.fn(() => "pending-id"),
  queuePendingVerification: vi.fn(),
  resolvePendingVerification: vi.fn(),
}));
vi.mock("@/lib/fix-guides", () => ({
  getFixGuide: vi.fn(() => null),
  getEffortLabel: vi.fn(() => "Small"),
}));

import { WebIssueDossier } from "./WebIssueDossier";
import { normalizeAppUrlForKey } from "@/lib/app-targets";

const ISSUE: CheckResult = {
  checkId: "security.csp",
  category: "security",
  title: "Content Security Policy is missing",
  description: "Responses do not include a CSP header.",
  status: "fail",
  severity: "high",
  fixPrompt: null,
  manualFix: "Add a Content-Security-Policy header to every HTML response.",
  rawData: { header: "missing" },
  confidence: "high",
  whyItMatters: "A missing CSP makes script injection easier.",
};

function renderDossier(overrides: Partial<React.ComponentProps<typeof WebIssueDossier>> = {}) {
  return render(
    <WebIssueDossier
      issue={ISSUE}
      projectId={7}
      url="https://example.com"
      projectPath="/tmp/project"
      onClose={vi.fn()}
      {...overrides}
    />,
  );
}

describe("WebIssueDossier", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
    agentActionProps.length = 0;
    actionBarProps.length = 0;
    verifyIssueMock.mockReset();
    verifyIssueMock.mockResolvedValue({ status: "queued", sources: ["gsc"] });
    toastError.mockReset();
    toastSuccess.mockReset();
  });

  it("renders the dossier with the agent fix action in the actions rail", () => {
    renderDossier();

    const dossier = screen.getByTestId("issue-dossier");
    expect(dossier).toHaveTextContent("Content Security Policy is missing");
    // Per-issue fix handoff goes through the agent loop, not a copy-prompt panel.
    expect(dossier).toHaveTextContent("FixWithAgentAction");

    const props = agentActionProps.at(-1)!;
    expect(props.projectId).toBe(7);
    expect(props.envUrl).toBe(normalizeAppUrlForKey("https://example.com"));
    expect(props.checkId).toBe("security.csp");
    expect(props.title).toBe("Content Security Policy is missing");
    expect(props.severity).toBe("high");
    expect(props.description).toBe("Responses do not include a CSP header.");
    expect(props.url).toBe("https://example.com");
    expect(props.whyItMatters).toBe("A missing CSP makes script injection easier.");
    expect(props.evidence).toEqual({ header: "missing" });
    expect(props.manualFix).toBe("Add a Content-Security-Policy header to every HTML response.");
    expect(props.previousFailure).toBeNull();
    // The dossier exposes no persistent retry signal.
    expect(props.openSignal).toBeUndefined();
    expect(props.projectPath).toBe("/tmp/project");
    expect(typeof props.onAttemptCreated).toBe("function");
  });

  it("adapts the dossier integrations callback for the agent handoff", () => {
    const onOpenIntegrations = vi.fn();
    renderDossier({ onOpenIntegrations });

    const props = agentActionProps.at(-1)!;
    expect(typeof props.onOpenIntegrations).toBe("function");
    (props.onOpenIntegrations as () => void)();
    expect(onOpenIntegrations).toHaveBeenCalledWith("");
  });

  it("omits the integrations link when the dossier has no integrations affordance", () => {
    renderDossier();

    expect(agentActionProps.at(-1)!.onOpenIntegrations).toBeUndefined();
  });

  it("combines the confidence label and rationale in one sidebar section", () => {
    renderDossier({
      issue: {
        ...ISSUE,
        confidence: "needs_review",
        confidenceReason:
          "The static response does not establish the effective header on every route.",
      },
    });

    const dossier = screen.getByTestId("issue-dossier");
    expect(dossier).toHaveTextContent("Needs review");
    expect(within(dossier).getAllByText("Confidence", { exact: true })).toHaveLength(1);
    expect(dossier).not.toHaveTextContent("Why this confidence");
    expect(dossier).toHaveTextContent(
      "The static response does not establish the effective header on every route.",
    );
  });

  it("routes Verify through the source-aware issue verifier", async () => {
    verifyIssueMock.mockResolvedValue({ status: "still_present", sources: ["web_scan"] });
    renderDossier();
    const verifyAction = actionBarProps.at(-1)!.verifyAction as { onClick: () => Promise<void> };

    await act(() => verifyAction.onClick());

    expect(verifyIssueMock).toHaveBeenCalledWith(
      7,
      normalizeAppUrlForKey("https://example.com"),
      "security.csp",
    );
    expect(invokeMock).not.toHaveBeenCalledWith("verify_scan_checks", expect.anything());
    expect(toastError).toHaveBeenCalledWith("Still present", expect.stringMatching(/fresh check/i));
  });

  it("uses verified state instead of converting a successful verification to Ignore", async () => {
    const onDismiss = vi.fn();
    const onClose = vi.fn();
    verifyIssueMock.mockResolvedValue({ status: "verified", sources: ["web_scan"] });
    renderDossier({ onDismiss, onClose });
    const verifyAction = actionBarProps.at(-1)!.verifyAction as { onClick: () => Promise<void> };

    await act(() => verifyAction.onClick());

    expect(onDismiss).toHaveBeenCalledWith("security.csp");
    expect(onClose).toHaveBeenCalledOnce();
    expect(toastSuccess).toHaveBeenCalledWith("Verified", expect.stringMatching(/fresh check/i));
    expect(invokeMock).not.toHaveBeenCalledWith(
      "ignore_issue",
      expect.objectContaining({ checkId: "security.csp" }),
    );
  });
});
