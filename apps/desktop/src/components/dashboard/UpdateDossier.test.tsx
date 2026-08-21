import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PackageUpdate } from "@/lib/types";

const { invokeMock, agentActionProps, copyToClipboardMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  agentActionProps: [] as Array<Record<string, unknown>>,
  copyToClipboardMock: vi.fn(() => Promise.resolve(true)),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
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
  IssueActionBar: ({ extraActions }: { extraActions?: React.ReactNode }) =>
    React.createElement("div", null, "IssueActionBar", extraActions),
}));
vi.mock("@/components/issues/FixWithAgentAction", () => ({
  FixWithAgentAction: (props: Record<string, unknown>) => {
    agentActionProps.push(props);
    return React.createElement("div", null, "FixWithAgentAction");
  },
}));
vi.mock("@/lib/clipboard", () => ({ copyToClipboard: copyToClipboardMock }));
vi.mock("@/lib/desktop-prompts", () => ({
  useDesktopPromptCenter: vi.fn(() => []),
}));
vi.mock("@/lib/update-memory", () => ({
  getUpdateMemory: vi.fn(() => null),
}));
vi.mock("@/lib/pending-verification", () => ({
  buildPendingVerificationId: vi.fn(() => "pending-id"),
  queuePendingVerification: vi.fn(),
  resolvePendingVerification: vi.fn(),
}));
import { UpdateDossier } from "./UpdateDossier";

function makeUpdate(overrides: Partial<PackageUpdate> = {}): PackageUpdate {
  return {
    name: "lodash",
    currentVersion: "4.17.20",
    latestVersion: "4.17.21",
    ecosystem: "npm",
    updateType: "patch",
    isSecurity: false,
    advisorySeverity: null,
    advisoryUrl: null,
    source: "package.json",
    isDev: false,
    isDeprecated: false,
    deprecationMessage: null,
    currentVersionDeprecated: false,
    isStale: false,
    lastPublished: null,
    workspaceMembers: [],
    ...overrides,
  };
}

function renderDossier(update: PackageUpdate, allUpdates: PackageUpdate[]) {
  return render(
    <UpdateDossier
      update={update}
      allUpdates={allUpdates}
      projectId={9}
      url="https://example.com"
      projectPath="/tmp/project"
      onClose={vi.fn()}
      onVerify={vi.fn()}
      verifying={false}
    />,
  );
}

describe("UpdateDossier agent fix action", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
    agentActionProps.length = 0;
  });

  it("dispatches a security update against the shared vulnerability group", () => {
    const selected = makeUpdate({
      isSecurity: true,
      advisorySeverity: "critical",
      advisoryFixedVersion: "4.17.21",
    });
    const otherVulnerable = makeUpdate({
      name: "minimist",
      currentVersion: "1.2.5",
      latestVersion: "1.2.8",
      isSecurity: true,
      advisorySeverity: "high",
      advisoryFixedVersion: "1.2.8",
    });
    renderDossier(selected, [selected, otherVulnerable]);

    expect(screen.getByTestId("issue-dossier")).toHaveTextContent("FixWithAgentAction");
    const props = agentActionProps.at(-1)!;
    expect(props.projectId).toBe(9);
    expect(props.envUrl).toBe("https://example.com");
    // The whole vulnerability group, because verification settles per group.
    expect(props.checkId).toBe("dependencies.vulnerability");
    expect(props.title).toBe("Vulnerabilities in 2 dependencies");
    expect(props.severity).toBe("critical");
    expect(props.description).toContain("minimist 1.2.5 -> 1.2.8");
    expect(props.manualFix).toContain("npm install minimist@1.2.8");
    expect(props.projectPath).toBe("/tmp/project");
    expect(typeof props.onAttemptCreated).toBe("function");
  });

  it("dispatches a major update against the outdated-major group", () => {
    const selected = makeUpdate({ updateType: "major", latestVersion: "5.0.0" });
    renderDossier(selected, [selected]);

    const props = agentActionProps.at(-1)!;
    expect(props.checkId).toBe("dependencies.outdated-major");
    expect(props.severity).toBe("low");
  });

  it("offers no agent action for patch updates, which have no work item to verify against", () => {
    renderDossier(makeUpdate({ updateType: "patch" }), []);

    expect(screen.getByTestId("issue-dossier")).not.toHaveTextContent("FixWithAgentAction");
    expect(agentActionProps).toHaveLength(0);
    // The identity-less dossier must not poll for fix attempts either.
    expect(invokeMock).not.toHaveBeenCalledWith("get_fix_attempt_for_issue", expect.anything());
  });
});

describe("UpdateDossier lean content", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
    agentActionProps.length = 0;
  });

  it("shows the advisory line for security updates", () => {
    renderDossier(
      makeUpdate({
        isSecurity: true,
        advisorySeverity: "high",
        advisoryFixedVersion: "4.17.21",
        advisoryUrl: "https://github.com/advisories/GHSA-xxxx",
      }),
      [],
    );
    const dossier = screen.getByTestId("issue-dossier");
    expect(dossier).toHaveTextContent("A verified fixed release is available (high severity)");
    expect(dossier).toHaveTextContent("https://github.com/advisories/GHSA-xxxx");
  });

  it("shows mitigation guidance when no fixed release is published", () => {
    renderDossier(makeUpdate({ isSecurity: true, advisorySeverity: "high" }), []);

    const dossier = screen.getByTestId("issue-dossier");
    expect(dossier).toHaveTextContent("No fixed release is published (high severity)");
    expect(dossier).toHaveTextContent("Review the advisory for reachable code paths");
    expect(screen.queryByRole("button", { name: /copy command/i })).not.toBeInTheDocument();
    expect(dossier).toHaveTextContent("FixWithAgentAction");
    expect(agentActionProps.at(-1)?.manualFix).toContain("remove, replace, or isolate");
  });

  it("omits the advisory line entirely for non-security updates", () => {
    renderDossier(makeUpdate({ updateType: "major" }), []);
    expect(screen.getByTestId("issue-dossier")).not.toHaveTextContent(/vulnerability/i);
  });

  it("never re-renders information the header already carries", () => {
    const selected = makeUpdate({ isSecurity: true, advisorySeverity: "critical" });
    const other = makeUpdate({ name: "minimist", isSecurity: true });
    renderDossier(selected, [selected, other]);

    const dossier = screen.getByTestId("issue-dossier");
    for (const removedLabel of [
      "Version change",
      "Update type",
      "Reason to prioritize",
      "Shared dependency surface",
      "Dependency memory",
      "How to check it",
    ]) {
      expect(dossier).not.toHaveTextContent(removedLabel);
    }
  });
});

describe("UpdateDossier command block", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
    copyToClipboardMock.mockClear();
  });

  it("copies the command from an icon on the command block", async () => {
    renderDossier(makeUpdate(), []);

    fireEvent.click(screen.getByRole("button", { name: "Copy command" }));

    await waitFor(() => {
      expect(copyToClipboardMock).toHaveBeenCalledWith("npm install lodash@4.17.21");
    });
    expect(await screen.findByRole("button", { name: "Command copied" })).toBeInTheDocument();
  });

  it("drops the editor, folder, and run-command actions", () => {
    renderDossier(makeUpdate(), []);

    const dossier = screen.getByTestId("issue-dossier");
    for (const removed of [
      "Open in editor",
      "Open changed file",
      "Reveal folder",
      "Run command",
      "Last command run",
    ]) {
      expect(dossier).not.toHaveTextContent(removed);
    }
    expect(screen.queryByRole("button", { name: /copy command/i })).toBeInTheDocument();
  });
});
