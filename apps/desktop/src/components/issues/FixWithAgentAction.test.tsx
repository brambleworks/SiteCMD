import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentTool, AgentToolStatus, FixAttempt } from "@/lib/fix-attempts";

/** Holds before and after the lazy Markdown renderer resolves. */
async function expectPromptShown(body: string) {
  await waitFor(() => {
    const block = document.querySelector(".fix-prompt-block");
    expect(block).not.toBeNull();
    expect(block).toHaveTextContent(body);
  });
}

const { invokeMock, copyToClipboardMock, toastSuccessMock, toastErrorMock, safeListenMock } =
  vi.hoisted(() => ({
    invokeMock: vi.fn(),
    copyToClipboardMock: vi.fn(() => Promise.resolve(true)),
    toastSuccessMock: vi.fn(),
    toastErrorMock: vi.fn(),
    safeListenMock: vi.fn((_event: string, _handler: (event: { payload: unknown }) => void) =>
      Promise.resolve(() => {}),
    ),
  }));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/lib/clipboard", () => ({ copyToClipboard: copyToClipboardMock }));
vi.mock("@/lib/tauri-events", () => ({ safeListen: safeListenMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: toastSuccessMock,
    error: toastErrorMock,
  }),
}));
import { FixWithAgentAction } from "@/components/issues/FixWithAgentAction";
import { resetFixHandoffStoreForTests } from "@/lib/fix-handoff-store";

function toolStatus(tool: AgentTool, registered: boolean): AgentToolStatus {
  return {
    tool,
    installed: true,
    registered,
    healthy: registered,
    needsRepair: false,
    repairReason: null,
    nodeAvailable: true,
    configPath: `/home/user/.${tool}.json`,
    plannedChange: "Add the sitecmd MCP server.",
  };
}

const attempt: FixAttempt = {
  id: 41,
  status: "briefed",
  agentTool: "claude-code",
  agentSummary: null,
  failureDetail: null,
  kickoffPrompt: "KICKOFF PROMPT BODY",
  briefFetchedAt: null,
  createdAt: 1700000000,
  updatedAt: 1700000000,
};

interface InvokeOptions {
  tools?: AgentToolStatus[];
  /** Resolved by the live-progress refetch in the handoff modal. */
  latestAttempt?: () => FixAttempt | null;
  launch?: () => Promise<void>;
  onCreate?: () => void;
}

function mockInvoke(options: InvokeOptions) {
  invokeMock.mockImplementation((command: string) => {
    if (command === "detect_agent_tools") return Promise.resolve(options.tools ?? []);
    if (command === "create_fix_attempt") {
      options.onCreate?.();
      return Promise.resolve(attempt);
    }
    if (command === "launch_agent_handoff") {
      return options.launch ? options.launch() : Promise.resolve(null);
    }
    if (command === "get_fix_attempt_for_issue") {
      return Promise.resolve(options.latestAttempt ? options.latestAttempt() : null);
    }
    return Promise.resolve(null);
  });
}

const baseProps = {
  projectId: 7,
  envUrl: "https://example.com",
  checkId: "missing_csp",
  title: "Missing Content Security Policy",
  severity: "high",
  description: "The site does not send a Content-Security-Policy header.",
  url: "https://example.com",
};

/** Click the main button with NO remembered tool: opens the setup modal. */
async function openSetupModal() {
  fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));
  await screen.findByRole("dialog");
}

const launchCalls = () =>
  invokeMock.mock.calls.filter(([command]) => command === "launch_agent_handoff");
const createCalls = () =>
  invokeMock.mock.calls.filter(([command]) => command === "create_fix_attempt");

beforeEach(() => {
  vi.clearAllMocks();
  safeListenMock.mockResolvedValue(() => {});
  window.localStorage.clear();
  resetFixHandoffStoreForTests();
});

describe("FixWithAgentAction setup modal", () => {
  it("opens the handoff modal listing registered tools", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", true), toolStatus("cursor", false)] });
    render(<FixWithAgentAction {...baseProps} />);

    await openSetupModal();

    expect(await screen.findByRole("radio", { name: "Claude Code" })).toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: "Cursor" })).not.toBeInTheDocument();
  });

  it("dispatches from Start fix: creates, launches, and shows live progress", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", true)] });
    render(<FixWithAgentAction {...baseProps} />);

    await openSetupModal();
    await screen.findByRole("radio", { name: "Claude Code" });
    fireEvent.click(screen.getByRole("button", { name: /Start fix/ }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_fix_attempt",
        expect.objectContaining({
          args: expect.objectContaining({
            agentTool: "claude-code",
            projectId: 7,
            checkId: "missing_csp",
          }),
        }),
      );
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "launch_agent_handoff",
        expect.objectContaining({
          tool: "claude-code",
          kickoffPrompt: "KICKOFF PROMPT BODY",
        }),
      );
    });
    // The modal stays open as the live progress view; the clipboard backup
    // still happens silently, with no toast (the modal is the feedback).
    expect(await screen.findByText("Your agent is on it")).toBeInTheDocument();
    expect(
      await screen.findByText("Claude Code opened with the fix prompt staged"),
    ).toBeInTheDocument();
    expect(copyToClipboardMock).toHaveBeenCalledWith("KICKOFF PROMPT BODY");
    expect(toastSuccessMock).not.toHaveBeenCalled();
  });

  it("falls back to copy-the-brief guidance when no tool is registered", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", false), toolStatus("cursor", false)] });
    const onOpenIntegrations = vi.fn();
    render(<FixWithAgentAction {...baseProps} onOpenIntegrations={onOpenIntegrations} />);

    await openSetupModal();

    expect(await screen.findByText(/No agent tools are connected/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Set up in Integrations" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Copy the fix prompt instead/ }));
    await waitFor(() => {
      expect(copyToClipboardMock).toHaveBeenCalledWith("KICKOFF PROMPT BODY");
    });
    // The modal stays open showing the prompt itself - never a copy button
    // for invisible content - and keeps tracking the manual paste loop.
    await expectPromptShown("KICKOFF PROMPT BODY");
    expect(
      screen.getByText("Fix prompt copied - paste it into your agent and send it"),
    ).toBeInTheDocument();
    expect(toastSuccessMock).not.toHaveBeenCalled();
  });

  it("shows the inline error and fires a toast when create fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_agent_tools") {
        return Promise.resolve([toolStatus("claude-code", true)]);
      }
      if (command === "create_fix_attempt") {
        return Promise.reject(new Error("brief generation failed"));
      }
      return Promise.resolve(null);
    });
    render(<FixWithAgentAction {...baseProps} />);

    await openSetupModal();
    await screen.findByRole("radio", { name: "Claude Code" });
    fireEvent.click(screen.getByRole("button", { name: /Start fix/ }));

    expect(await screen.findByText("Brief generation failed.")).toBeInTheDocument();
    expect(toastErrorMock).toHaveBeenCalledWith(
      "Could not start the fix",
      "Brief generation failed.",
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(launchCalls()).toHaveLength(0);
  });

  it("opens the modal only when openSignal increments past its mount value", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", true)] });
    const detectCalls = () =>
      invokeMock.mock.calls.filter(([command]) => command === "detect_agent_tools").length;

    const { rerender } = render(<FixWithAgentAction {...baseProps} openSignal={0} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    rerender(<FixWithAgentAction {...baseProps} openSignal={1} />);
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    // One detect on mount (for the per-agent buttons) plus one for this open.
    await waitFor(() => expect(detectCalls()).toBe(2));

    // Re-rendering with the same signal must not re-trigger the open flow.
    rerender(<FixWithAgentAction {...baseProps} openSignal={1} />);
    expect(detectCalls()).toBe(2);
  });

  it("does not auto-open on mount with a stale non-zero openSignal", () => {
    mockInvoke({ tools: [toolStatus("claude-code", true)] });
    render(<FixWithAgentAction {...baseProps} openSignal={3} />);

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    // Detection may run on mount for the per-agent buttons, but a stale signal
    // must never dispatch a fix.
    expect(createCalls()).toHaveLength(0);
    expect(launchCalls()).toHaveLength(0);
  });

  it("shows a distinct message when tool detection fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_agent_tools") {
        return Promise.reject(new Error("backend unavailable"));
      }
      if (command === "create_fix_attempt") return Promise.resolve(attempt);
      return Promise.resolve(null);
    });
    const onOpenIntegrations = vi.fn();
    render(<FixWithAgentAction {...baseProps} onOpenIntegrations={onOpenIntegrations} />);

    await openSetupModal();

    expect(await screen.findByText("Could not check for agent tools.")).toBeInTheDocument();
    expect(screen.queryByText(/No agent tools are connected/i)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Set up in Integrations" })).toBeInTheDocument();
  });
});

describe("one-click handoff with a remembered tool", () => {
  it("launches the agent and shows the live progress modal in one click", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    mockInvoke({ tools: [toolStatus("claude-code", true)] });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));

    expect(await screen.findByText("Your agent is on it")).toBeInTheDocument();
    await waitFor(() => expect(createCalls()).toHaveLength(1));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "launch_agent_handoff",
        expect.objectContaining({
          tool: "claude-code",
          kickoffPrompt: "KICKOFF PROMPT BODY",
        }),
      ),
    );
    // No tool picker in the one-click path; the steps render instead.
    expect(screen.queryByRole("radio", { name: "Claude Code" })).not.toBeInTheDocument();
    expect(await screen.findByText("Fix brief prepared")).toBeInTheDocument();
  });

  it("takes the one-click path for a remembered Windsurf choice", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "windsurf");
    mockInvoke({ tools: [toolStatus("windsurf", true)] });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    // No tool picker in the one-click path; Windsurf has no deep link, so the
    // handoff lands directly on the manual copy-paste guidance.
    expect(screen.queryByRole("radio", { name: "Windsurf" })).not.toBeInTheDocument();
    expect(
      await screen.findByText("Fix prompt copied - paste it into Windsurf and send it"),
    ).toBeInTheDocument();
    expect(launchCalls()).toHaveLength(0);
  });

  it("falls back to the setup modal when the remembered tool is not registered", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "cursor");
    mockInvoke({ tools: [toolStatus("claude-code", true), toolStatus("cursor", false)] });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));

    expect(await screen.findByRole("radio", { name: "Claude Code" })).toBeInTheDocument();
    expect(createCalls()).toHaveLength(0);
    expect(launchCalls()).toHaveLength(0);
  });

  it("change tool opens the setup picker with the remembered tool preselected", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "cursor");
    mockInvoke({ tools: [toolStatus("claude-code", true), toolStatus("cursor", true)] });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));
    await screen.findByText("Your agent is on it");

    fireEvent.click(screen.getByRole("button", { name: "change tool" }));

    const cursorRadio = await screen.findByRole("radio", { name: "Cursor" });
    expect(cursorRadio).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("radio", { name: "Claude Code" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  it("shows paste-it-yourself guidance when the deep link cannot launch", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    mockInvoke({
      tools: [toolStatus("claude-code", true)],
      launch: () => Promise.reject(new Error("no handler for claude://")),
    });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));

    expect(await screen.findByText(/Could not open Claude Code/)).toBeInTheDocument();
    // The prompt is shown in full next to the copy button, not copy-only.
    await expectPromptShown("KICKOFF PROMPT BODY");

    fireEvent.click(screen.getByRole("button", { name: "Copy fix prompt" }));
    await waitFor(() => {
      expect(copyToClipboardMock).toHaveBeenLastCalledWith("KICKOFF PROMPT BODY");
    });
  });

  it("copies the prompt instead of failing when the editor has no deep link", async () => {
    mockInvoke({
      tools: [toolStatus("windsurf", true)],
      launch: () =>
        Promise.reject(
          new Error(
            "Windsurf has no prompt deep link. The kickoff prompt is on your clipboard; paste it into the agent.",
          ),
        ),
    });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(await screen.findByRole("button", { name: "Fix with Windsurf" }));

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(
      await screen.findByText("Fix prompt copied - paste it into Windsurf and send it"),
    ).toBeInTheDocument();
    await expectPromptShown("KICKOFF PROMPT BODY");
    expect(screen.queryByText(/Could not open Windsurf/)).not.toBeInTheDocument();
    expect(launchCalls()).toHaveLength(0);
    expect(copyToClipboardMock).toHaveBeenCalledWith("KICKOFF PROMPT BODY");
  });

  it("advances the progress steps as the attempt moves through the loop", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    let latest: FixAttempt = attempt;
    mockInvoke({
      tools: [toolStatus("claude-code", true)],
      latestAttempt: () => latest,
    });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));
    // Wait for the attempt and progress listener to initialize.
    await waitFor(() => expect(launchCalls()).toHaveLength(1));
    await act(async () => {});

    // The MCP server stamps the pickup; the watcher event drives the refetch.
    latest = { ...attempt, briefFetchedAt: 1700000500 };
    const progressListener = safeListenMock.mock.calls.find(
      ([event]) => event === "fix-attempt-updated",
    );
    expect(progressListener).toBeDefined();
    await act(async () => {
      // Every registered listener refetches; fire them all like the real event.
      for (const [, handler] of safeListenMock.mock.calls) handler({ payload: undefined });
    });
    expect(await screen.findByText("Claude Code picked up the brief")).toBeInTheDocument();

    latest = { ...attempt, briefFetchedAt: 1700000500, status: "verified" };
    await act(async () => {
      for (const [, handler] of safeListenMock.mock.calls) handler({ payload: undefined });
    });
    expect(
      await screen.findByText("Fix verified. SiteCMD re-ran the check and it passes now."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Done" })).toBeInTheDocument();
  });

  it("offers a retry with the failure context when verification fails", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    let latest: FixAttempt = attempt;
    mockInvoke({
      tools: [toolStatus("claude-code", true)],
      latestAttempt: () => latest,
    });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));
    // Wait for the attempt and progress listener to initialize.
    await waitFor(() => expect(launchCalls()).toHaveLength(1));
    await act(async () => {});

    latest = {
      ...attempt,
      status: "verify_failed",
      failureDetail: "The CSP header is still missing.",
    };
    await act(async () => {
      for (const [, handler] of safeListenMock.mock.calls) handler({ payload: undefined });
    });

    expect(await screen.findByText("The CSP header is still missing.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));

    await waitFor(() => expect(createCalls()).toHaveLength(2));
    const retryArgs = createCalls()[1][1] as { args: { previousFailure: string | null } };
    expect(retryArgs.args.previousFailure).toBe("The CSP header is still missing.");
  });

  it("ignores clicks outside the modal; only explicit controls close it", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    mockInvoke({ tools: [toolStatus("claude-code", true)], latestAttempt: () => attempt });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));
    await screen.findByText("Your agent is on it");

    const dialog = screen.getByRole("dialog", { name: "Your agent is on it" });
    fireEvent.click(dialog);
    expect(screen.getByText("Your agent is on it")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByText("Your agent is on it")).not.toBeInTheDocument();
  });

  it("explains the deploy wait while a remote web check verifies", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    let latest: FixAttempt = attempt;
    mockInvoke({ tools: [toolStatus("claude-code", true)], latestAttempt: () => latest });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));
    await waitFor(() => expect(launchCalls()).toHaveLength(1));
    await act(async () => {});
    expect(screen.queryByText(/verifies the live site/)).not.toBeInTheDocument();

    latest = { ...attempt, briefFetchedAt: 1700000500, status: "verifying" };
    await act(async () => {
      for (const [, handler] of safeListenMock.mock.calls) handler({ payload: undefined });
    });
    expect(await screen.findByText(/deploy them - SiteCMD keeps re-checking/)).toBeInTheDocument();
  });

  it("skips the deploy note for local environments", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    let latest: FixAttempt = attempt;
    mockInvoke({ tools: [toolStatus("claude-code", true)], latestAttempt: () => latest });
    render(<FixWithAgentAction {...baseProps} envUrl="http://localhost:4321" />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));
    await waitFor(() => expect(launchCalls()).toHaveLength(1));

    latest = { ...attempt, status: "verifying" };
    await act(async () => {
      for (const [, handler] of safeListenMock.mock.calls) handler({ payload: undefined });
    });
    await screen.findByText("Your agent is on it");
    expect(screen.queryByText(/verifies the live site/)).not.toBeInTheDocument();
  });

  it("keeps the progress modal open across a component remount", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    mockInvoke({ tools: [toolStatus("claude-code", true)], latestAttempt: () => attempt });
    const { unmount } = render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Fix with your agent" }));
    await waitFor(() => expect(launchCalls()).toHaveLength(1));
    await act(async () => {});
    expect(screen.getByText("Your agent is on it")).toBeInTheDocument();

    unmount();
    expect(screen.queryByText("Your agent is on it")).not.toBeInTheDocument();

    render(<FixWithAgentAction {...baseProps} />);

    expect(await screen.findByText("Your agent is on it")).toBeInTheDocument();
    expect(await screen.findByText("Fix brief prepared")).toBeInTheDocument();
    expect(createCalls()).toHaveLength(1);
    expect(launchCalls()).toHaveLength(1);
  });
});

describe("per-agent fix buttons", () => {
  it("renders one dispatch button per connected agent", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", true), toolStatus("codex", true)] });
    render(<FixWithAgentAction {...baseProps} />);

    expect(await screen.findByRole("button", { name: "Fix with Claude Code" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Fix with Codex" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Fix with your agent" })).not.toBeInTheDocument();
  });

  it("dispatches the agent whose button was clicked, not the first-detected one", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", true), toolStatus("codex", true)] });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(await screen.findByRole("button", { name: "Fix with Codex" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_fix_attempt",
        expect.objectContaining({ args: expect.objectContaining({ agentTool: "codex" }) }),
      );
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "launch_agent_handoff",
        expect.objectContaining({ tool: "codex" }),
      );
    });
    // Claude Code (first in the detection order) must not have been dispatched.
    expect(
      createCalls().some(([, payload]) => {
        const args = (payload as { args?: { agentTool?: string } }).args;
        return args?.agentTool === "claude-code";
      }),
    ).toBe(false);
  });

  it("omits agents that are installed but not registered", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", true), toolStatus("codex", false)] });
    render(<FixWithAgentAction {...baseProps} />);

    expect(await screen.findByRole("button", { name: "Fix with Claude Code" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Fix with Codex" })).not.toBeInTheDocument();
  });

  it("omits registered agents whose MCP runtime needs repair", async () => {
    const stale = {
      ...toolStatus("claude-code", true),
      healthy: false,
      needsRepair: true,
      repairReason: "The saved MCP command is stale",
    };
    mockInvoke({ tools: [stale] });
    render(<FixWithAgentAction {...baseProps} />);

    await openSetupModal();
    expect(await screen.findByText(/No agent tools are connected/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Fix with Claude Code" })).not.toBeInTheDocument();
  });

  it("falls back to the generic setup button when no agent is connected", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", false)] });
    render(<FixWithAgentAction {...baseProps} />);

    // Detection resolves with nothing registered: the setup button stays and
    // opens the picker/copy-brief modal.
    await openSetupModal();
    expect(await screen.findByText(/No agent tools are connected/i)).toBeInTheDocument();
  });
});

describe("FixWithAgentAction retired fix meter", () => {
  const allowanceCalls = () =>
    invokeMock.mock.calls.filter(([command]) => command === "get_fix_allowance").length;

  it("renders the dispatch buttons immediately with no allowance fetch", async () => {
    mockInvoke({ tools: [toolStatus("claude-code", true)] });
    render(<FixWithAgentAction {...baseProps} />);

    // The generic button is up before detection resolves; a per-agent button
    // replaces it after. Neither waits on a meter.
    expect(screen.getByRole("button", { name: "Fix with your agent" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Fix with Claude Code" })).toBeInTheDocument();
    expect(allowanceCalls()).toBe(0);
    expect(screen.queryByText(/free fixes/i)).not.toBeInTheDocument();
  });

  it("completes a full dispatch without ever consulting a meter", async () => {
    window.localStorage.setItem("sitecmd:agent-tool", "claude-code");
    mockInvoke({ tools: [toolStatus("claude-code", true)] });
    render(<FixWithAgentAction {...baseProps} />);

    fireEvent.click(await screen.findByRole("button", { name: "Fix with Claude Code" }));

    expect(await screen.findByText("Your agent is on it")).toBeInTheDocument();
    await waitFor(() => expect(createCalls()).toHaveLength(1));
    await waitFor(() => expect(launchCalls()).toHaveLength(1));

    expect(allowanceCalls()).toBe(0);
    expect(screen.queryByText(/free fixes/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/You've used all/)).not.toBeInTheDocument();
  });
});
