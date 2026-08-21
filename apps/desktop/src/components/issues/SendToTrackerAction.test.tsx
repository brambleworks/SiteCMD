import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CheckResult, IssueLink } from "@/lib/types";
import { SendToTrackerAction } from "./SendToTrackerAction";
import { withQueryClient } from "@/test-utils/query-client";

const {
  toastSuccess,
  toastError,
  getIssueLinkForCheckMock,
  getIntegrationsMock,
  createIssueLinkMock,
  getProjectsMock,
  openUrlMock,
} = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  getIssueLinkForCheckMock: vi.fn(),
  getIntegrationsMock: vi.fn(),
  createIssueLinkMock: vi.fn(),
  getProjectsMock: vi.fn(),
  openUrlMock: vi.fn(),
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ success: toastSuccess, warning: vi.fn(), error: toastError }),
}));
vi.mock("@/lib/commands", () => ({
  getIssueLinkForCheck: getIssueLinkForCheckMock,
  getIntegrations: getIntegrationsMock,
  createIssueLink: createIssueLinkMock,
  getProjects: getProjectsMock,
}));
vi.mock("@/lib/open-url", () => ({ openUrl: openUrlMock }));

const ISSUE: CheckResult = {
  checkId: "security.csp",
  category: "security",
  title: "Content Security Policy is missing",
  description: "Responses do not include a CSP header.",
  status: "fail",
  severity: "high",
  fixPrompt: null,
  manualFix: null,
  rawData: null,
  confidence: "high",
};

const LINK: IssueLink = {
  id: 1,
  projectId: 1,
  checkId: "security.csp",
  scanId: 42,
  provider: "github",
  externalId: "#12",
  externalUrl: "https://github.com/acme/site/issues/12",
  status: "open",
  createdAt: "2026-07-13T10:00:00Z",
  resolvedAt: null,
};

function githubIntegration(enabled = true) {
  return { integrationType: "github", apiKey: null, siteId: null, extra: null, enabled };
}
function jiraIntegration(enabled = true) {
  return { integrationType: "jira", apiKey: null, siteId: null, extra: null, enabled };
}

function renderAction(overrides: Partial<React.ComponentProps<typeof SendToTrackerAction>> = {}) {
  return render(
    <SendToTrackerAction
      projectId={1}
      issue={ISSUE}
      scanId={42}
      estimatedImpact={4}
      {...overrides}
    />,
    { wrapper: withQueryClient() },
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  getIssueLinkForCheckMock.mockResolvedValue(null);
  getIntegrationsMock.mockResolvedValue([]);
  getProjectsMock.mockResolvedValue([{ id: 1, name: "Acme Site" }]);
});

describe("SendToTrackerAction", () => {
  it("renders a send button per enabled tracker integration", async () => {
    getIntegrationsMock.mockResolvedValue([githubIntegration(), jiraIntegration()]);
    renderAction();
    expect(await screen.findByRole("button", { name: "Send to GitHub" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send to Jira" })).toBeInTheDocument();
  });

  it("renders nothing when no tracker is configured", async () => {
    const { container } = renderAction();
    await waitFor(() => expect(getIntegrationsMock).toHaveBeenCalled());
    await waitFor(() => expect(container.firstChild).toBeNull());
    expect(
      screen.queryByRole("button", { name: /Connect GitHub or Jira/ }),
    ).not.toBeInTheDocument();
  });

  it("hides the send buttons until the issue belongs to a stored scan", async () => {
    getIntegrationsMock.mockResolvedValue([githubIntegration()]);
    const { container } = renderAction({ scanId: null });
    await waitFor(() => expect(getIntegrationsMock).toHaveBeenCalled());
    await waitFor(() => expect(container.firstChild).toBeNull());
    expect(screen.queryByText(/upgrade/i)).not.toBeInTheDocument();
  });

  it("renders the existing ticket and opens it externally", async () => {
    getIssueLinkForCheckMock.mockResolvedValue(LINK);
    getIntegrationsMock.mockResolvedValue([githubIntegration()]);
    renderAction();
    const ticket = await screen.findByRole("button", { name: "Open GitHub ticket #12" });
    fireEvent.click(ticket);
    expect(openUrlMock).toHaveBeenCalledWith("https://github.com/acme/site/issues/12");
    expect(screen.queryByRole("button", { name: "Send to GitHub" })).not.toBeInTheDocument();
  });

  it("creates the ticket and swaps the button for the link", async () => {
    getIntegrationsMock.mockResolvedValue([githubIntegration()]);
    createIssueLinkMock.mockResolvedValue(LINK);
    const onLinkCreated = vi.fn();
    renderAction({ onLinkCreated });

    fireEvent.click(await screen.findByRole("button", { name: "Send to GitHub" }));

    expect(
      await screen.findByRole("button", { name: "Open GitHub ticket #12" }),
    ).toBeInTheDocument();
    expect(createIssueLinkMock).toHaveBeenCalledWith({
      projectId: 1,
      checkId: "security.csp",
      scanId: 42,
      provider: "github",
      estimatedImpact: 4,
    });
    expect(onLinkCreated).toHaveBeenCalledWith(LINK);
    expect(toastSuccess).toHaveBeenCalled();
  });

  it("surfaces a toast and keeps the send button when creation fails", async () => {
    getIntegrationsMock.mockResolvedValue([githubIntegration()]);
    createIssueLinkMock.mockRejectedValue(new Error("GitHub create issue returned 401"));
    renderAction();

    fireEvent.click(await screen.findByRole("button", { name: "Send to GitHub" }));

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    const button = screen.getByRole("button", { name: "Send to GitHub" });
    expect(button).toBeEnabled();
  });

  it("labels lookup failures instead of treating them as no configured tracker", async () => {
    getIssueLinkForCheckMock.mockRejectedValue(new Error("database unavailable"));

    renderAction();

    expect(await screen.findByText("Ticket status could not load.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
