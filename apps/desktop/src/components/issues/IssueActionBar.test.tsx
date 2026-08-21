import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { IssueActionBar } from "./IssueActionBar";

const tauriEventsMock = vi.hoisted(() => ({
  safeListen: vi.fn((_event: string, _handler: (event: { payload: unknown }) => void) =>
    Promise.resolve(() => {}),
  ),
}));
vi.mock("@/lib/tauri-events", () => tauriEventsMock);

const issuesMock = vi.hoisted(() => ({
  blockIssue: vi.fn().mockResolvedValue(undefined),
  ignoreIssue: vi.fn().mockResolvedValue(undefined),
  reopenIssue: vi.fn().mockResolvedValue(undefined),
  getIssueState: vi.fn().mockResolvedValue(null),
}));
vi.mock("@/lib/issues", () => issuesMock);

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ error: vi.fn(), success: vi.fn() }),
}));

describe("IssueActionBar", () => {
  it("renders Verify fix when verifyAction is provided", () => {
    render(
      <IssueActionBar projectId={1} verifyAction={{ label: "Verify fix", onClick: vi.fn() }} />,
    );
    expect(screen.getByRole("button", { name: /verify fix/i })).toBeInTheDocument();
  });

  it("renders Ignore and Block when not paused", () => {
    render(<IssueActionBar projectId={1} onIgnore={vi.fn()} onBlock={vi.fn()} />);
    const ignore = screen.getByRole("button", { name: /^ignore$/i });
    const block = screen.getByRole("button", { name: /^block$/i });
    const lifecycleRow = ignore.closest(".issue-action-lifecycle");

    expect(ignore).toBeInTheDocument();
    expect(block).toBeInTheDocument();
    expect(lifecycleRow).not.toBeNull();
    expect(lifecycleRow).toContainElement(block);
    expect(ignore).toHaveAttribute("title", expect.stringMatching(/until the next scan/i));
    expect(block).toHaveAttribute("title", expect.stringMatching(/across future scans/i));
    expect(block).toHaveAttribute("title", expect.stringMatching(/until you restore/i));
  });

  it("explains the different Ignore and Block lifetimes", () => {
    render(<IssueActionBar projectId={1} onIgnore={vi.fn()} onBlock={vi.fn()} />);
    expect(
      screen.getByText(/counts only active issues/i, { selector: ".issue-action-help" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/ignore removes this finding until the next scan/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/block keeps it out across future scans/i)).toBeInTheDocument();
  });

  it("does not show the triage score note once the issue is already paused", () => {
    render(<IssueActionBar projectId={1} initialStatus="ignored" onReopen={vi.fn()} />);
    expect(screen.queryByText(/counts only active issues/i)).not.toBeInTheDocument();
  });

  it("renders only Reopen when initialStatus is ignored", async () => {
    const onReopen = vi.fn();
    render(
      <IssueActionBar
        projectId={1}
        initialStatus="ignored"
        onIgnore={vi.fn()}
        onBlock={vi.fn()}
        onReopen={onReopen}
      />,
    );
    expect(screen.getByRole("button", { name: /reopen/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^ignore$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^block$/i })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /reopen/i }));
    await waitFor(() => expect(onReopen).toHaveBeenCalled());
  });

  it("renders only Reopen when initialStatus is blocked", () => {
    render(<IssueActionBar projectId={1} initialStatus="blocked" onReopen={vi.fn()} />);
    expect(screen.getByRole("button", { name: /reopen/i })).toBeInTheDocument();
  });

  it("does not render a Working button", () => {
    render(<IssueActionBar projectId={1} onIgnore={vi.fn()} />);
    expect(screen.queryByRole("button", { name: /working/i })).not.toBeInTheDocument();
  });

  it("does not render a manual Mark fixed button", () => {
    render(<IssueActionBar projectId={1} verifyAction={{ label: "Verify", onClick: vi.fn() }} />);
    expect(screen.queryByRole("button", { name: /mark fixed/i })).not.toBeInTheDocument();
  });
});

describe("IssueActionBar issue_states mode (check_id + env_url)", () => {
  beforeEach(() => {
    issuesMock.blockIssue.mockClear().mockResolvedValue(undefined);
    issuesMock.ignoreIssue.mockClear().mockResolvedValue(undefined);
    issuesMock.reopenIssue.mockClear().mockResolvedValue(undefined);
    issuesMock.getIssueState.mockClear().mockResolvedValue(null);
    tauriEventsMock.safeListen.mockClear();
  });

  it("persists Block via blockIssue(check_id, env_url)", async () => {
    render(
      <IssueActionBar
        projectId={7}
        checkId="code_scan.supply_chain_typosquat"
        envUrl="https://example.com"
        onBlock={vi.fn()}
      />,
    );
    const block = await screen.findByRole("button", { name: /^block$/i });
    fireEvent.click(block);

    await waitFor(() =>
      expect(issuesMock.blockIssue).toHaveBeenCalledWith(
        7,
        "https://example.com",
        "code_scan.supply_chain_typosquat",
        expect.any(String),
      ),
    );
  });

  it("persists Ignore via ignoreIssue(check_id, env_url)", async () => {
    render(
      <IssueActionBar
        projectId={7}
        checkId="security.csp"
        envUrl="https://example.com"
        onIgnore={vi.fn()}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: /^ignore$/i }));

    await waitFor(() =>
      expect(issuesMock.ignoreIssue).toHaveBeenCalledWith(7, "https://example.com", "security.csp"),
    );
  });

  it("hydrates the Reopen action from a blocked getIssueState", async () => {
    issuesMock.getIssueState.mockResolvedValueOnce(["blocked", null, "intended alias", null]);
    render(
      <IssueActionBar
        projectId={7}
        checkId="security.csp"
        envUrl="https://example.com"
        onReopen={vi.fn()}
      />,
    );

    // A paused (blocked) issue surfaces Reopen instead of Ignore/Block.
    const reopen = await screen.findByRole("button", { name: /reopen/i });
    fireEvent.click(reopen);
    await waitFor(() =>
      expect(issuesMock.reopenIssue).toHaveBeenCalledWith(7, "https://example.com", "security.csp"),
    );
  });

  it("rehydrates lifecycle state when site-score-changed lands for the project", async () => {
    issuesMock.getIssueState.mockResolvedValueOnce(null);
    render(
      <IssueActionBar
        projectId={7}
        checkId="code_scan.external-call-retry"
        envUrl="https://example.com"
      />,
    );
    await waitFor(() => expect(issuesMock.getIssueState).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("button", { name: /reopen/i })).not.toBeInTheDocument();

    issuesMock.getIssueState.mockResolvedValue(["blocked", null, null, null]);
    const scoreListener = tauriEventsMock.safeListen.mock.calls.find(
      ([event]) => event === "site-score-changed",
    );
    expect(scoreListener).toBeTruthy();
    act(() => {
      scoreListener![1]({ payload: { projectId: 7 } });
    });

    expect(await screen.findByRole("button", { name: /reopen/i })).toBeInTheDocument();
  });

  it("ignores site-score-changed events for other projects", async () => {
    issuesMock.getIssueState.mockResolvedValue(null);
    render(<IssueActionBar projectId={7} checkId="security.csp" envUrl="https://example.com" />);
    await waitFor(() => expect(issuesMock.getIssueState).toHaveBeenCalledTimes(1));

    const scoreListener = tauriEventsMock.safeListen.mock.calls.find(
      ([event]) => event === "site-score-changed",
    );
    expect(scoreListener).toBeTruthy();
    act(() => {
      scoreListener![1]({ payload: { projectId: 8 } });
    });

    await waitFor(() => expect(issuesMock.getIssueState).toHaveBeenCalledTimes(1));
  });

  it("shows a retryable error when lifecycle hydration fails", async () => {
    issuesMock.getIssueState.mockRejectedValue(new Error("database unavailable"));

    render(<IssueActionBar projectId={7} checkId="security.csp" envUrl="https://example.com" />);

    expect(await screen.findByText(/Issue status could not load/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });

  it("treats an unknown persisted lifecycle value as a read error", async () => {
    issuesMock.getIssueState.mockResolvedValue(["mystery", null, null, null]);

    render(<IssueActionBar projectId={7} checkId="security.csp" envUrl="https://example.com" />);

    expect(await screen.findByText(/Issue status could not load/i)).toBeInTheDocument();
  });
});
