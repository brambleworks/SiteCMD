import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  }),
}));

import { AgentToolCards } from "./AgentToolCards";
import type { AgentToolStatus } from "@/lib/fix-attempts";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

function renderAgentTools(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

const PLANNED_CHANGE = 'Add a "sitecmd" entry to mcpServers in ~/.claude.json';

function toolStatus(
  overrides: Partial<AgentToolStatus> & { tool: AgentToolStatus["tool"] },
): AgentToolStatus {
  const registered = overrides.registered ?? false;
  return {
    installed: false,
    registered,
    healthy: overrides.healthy ?? registered,
    needsRepair: overrides.needsRepair ?? false,
    repairReason: overrides.repairReason ?? null,
    nodeAvailable: true,
    configPath: "~/.claude.json",
    plannedChange: PLANNED_CHANGE,
    ...overrides,
  };
}

function mockDetect(statuses: AgentToolStatus[]) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "detect_agent_tools") return statuses;
    return null;
  });
}

/** The row <button> whose title matches. */
function rowByName(name: string): HTMLElement {
  const button = screen.getByText(name).closest("button");
  if (!button) throw new Error(`No agent-tool row for "${name}"`);
  return button;
}

describe("AgentToolCards", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders a row per tool: installed ones connect, not-installed ones are disabled", async () => {
    mockDetect([
      toolStatus({ tool: "claude-code", installed: true }),
      toolStatus({ tool: "codex", installed: false }),
    ]);

    renderAgentTools(<AgentToolCards />);

    expect(await screen.findByText("Claude Code")).toBeInTheDocument();
    const claudeRow = rowByName("Claude Code");
    const codexRow = rowByName("Codex");
    expect(claudeRow).toHaveTextContent("Connect");
    expect(claudeRow).toBeEnabled();
    expect(within(claudeRow).queryByLabelText("Connected")).toBeNull();
    // Not installed: the row is disabled.
    expect(codexRow).toBeDisabled();
  });

  it("paints cached tool status while revalidating it on remount", async () => {
    let resolveSecondRead!: (statuses: AgentToolStatus[]) => void;
    const secondRead = new Promise<AgentToolStatus[]>((resolve) => {
      resolveSecondRead = resolve;
    });
    let reads = 0;
    invokeMock.mockImplementation(async (command: string) => {
      if (command !== "detect_agent_tools") return null;
      reads += 1;
      if (reads === 1) {
        return [toolStatus({ tool: "claude-code", installed: true, registered: false })];
      }
      return secondRead;
    });
    const queryClient = createTestQueryClient();
    const first = render(<AgentToolCards />, { wrapper: withQueryClient(queryClient) });
    expect(await screen.findByText("Claude Code")).toBeInTheDocument();
    expect(rowByName("Claude Code")).toHaveTextContent("Connect");
    first.unmount();

    render(<AgentToolCards />, { wrapper: withQueryClient(queryClient) });

    // The cached row remains available while the filesystem detection is in
    // flight, so revisiting Settings does not flash the full skeleton.
    expect(rowByName("Claude Code")).toHaveTextContent("Connect");
    expect(screen.queryByLabelText("Agent tools loading state")).not.toBeInTheDocument();

    await act(async () => {
      resolveSecondRead([toolStatus({ tool: "claude-code", installed: true, registered: true })]);
    });
    await waitFor(() => {
      expect(rowByName("Claude Code")).toHaveTextContent("Manage");
    });
    expect(reads).toBe(2);
  });

  it("shows the exact planned change in the modal before registering", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_agent_tools") {
        return [toolStatus({ tool: "claude-code", installed: true })];
      }
      if (command === "register_agent_tool") {
        return toolStatus({ tool: "claude-code", installed: true, registered: true });
      }
      return null;
    });

    renderAgentTools(<AgentToolCards />);

    fireEvent.click(await screen.findByText("Claude Code"));

    expect(screen.getByText(PLANNED_CHANGE)).toBeInTheDocument();
    expect(
      screen.getByText("SiteCMD never edits this file without this confirmation."),
    ).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("register_agent_tool", expect.anything());

    fireEvent.click(screen.getByRole("button", { name: "Connect Claude Code" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("register_agent_tool", { tool: "claude-code" });
    });
    // The modal closes and the row shows a connected dot + Manage.
    await waitFor(() => {
      const row = rowByName("Claude Code");
      expect(within(row).getByLabelText("Connected")).toBeInTheDocument();
      expect(row).toHaveTextContent("Manage");
    });
  });

  it("shows a stale registration as Repair and verifies it before reconnecting", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_agent_tools") {
        return [
          toolStatus({
            tool: "codex",
            installed: true,
            registered: true,
            healthy: false,
            needsRepair: true,
            repairReason: "The saved SiteCMD MCP command is stale",
          }),
        ];
      }
      if (command === "register_agent_tool") {
        return toolStatus({ tool: "codex", installed: true, registered: true, healthy: true });
      }
      return null;
    });

    renderAgentTools(<AgentToolCards />);

    await screen.findByText("Codex");
    const row = rowByName("Codex");
    expect(row).toHaveTextContent("Repair");
    expect(within(row).queryByLabelText("Connected")).toBeNull();
    fireEvent.click(row);
    expect(screen.getByText("The saved SiteCMD MCP command is stale")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Repair Codex" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("register_agent_tool", { tool: "codex" });
    });
    await waitFor(() => expect(rowByName("Codex")).toHaveTextContent("Manage"));
  });

  it("disconnect from the modal calls unregister", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_agent_tools") {
        return [toolStatus({ tool: "cursor", installed: true, registered: true })];
      }
      if (command === "unregister_agent_tool") {
        return toolStatus({ tool: "cursor", installed: true, registered: false });
      }
      return null;
    });

    renderAgentTools(<AgentToolCards />);

    const cursorRow = (await screen.findByText("Cursor")).closest("button")!;
    expect(cursorRow).toHaveTextContent("Manage");
    fireEvent.click(cursorRow);
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("unregister_agent_tool", { tool: "cursor" });
    });
    await waitFor(() => {
      const row = rowByName("Cursor");
      expect(row).toHaveTextContent("Connect");
      expect(within(row).queryByLabelText("Connected")).toBeNull();
    });
  });

  it("node missing shows the shared notice and disables the row", async () => {
    mockDetect([toolStatus({ tool: "claude-code", installed: true, nodeAvailable: false })]);

    renderAgentTools(<AgentToolCards />);

    expect(
      await screen.findByText(
        "These connections run SiteCMD's agent connector with Node.js, which needs Node 22.22.1 or newer on your PATH. Install or update Node, then try again.",
      ),
    ).toBeInTheDocument();
    expect(rowByName("Claude Code")).toBeDisabled();
  });

  it("detect failure shows the error and Retry re-runs detection", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_agent_tools")
        return [toolStatus({ tool: "claude-code", installed: true })];
      return null;
    });
    invokeMock.mockRejectedValueOnce(new Error("detection blew up"));

    renderAgentTools(<AgentToolCards />);

    expect(await screen.findByText("Error: detection blew up")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    expect(await screen.findByText("Claude Code")).toBeInTheDocument();
    expect(screen.queryByText("Error: detection blew up")).not.toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(invokeMock).toHaveBeenNthCalledWith(2, "detect_agent_tools");
  });

  it("register rejection shows the error and keeps the modal open", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_agent_tools") {
        return [toolStatus({ tool: "claude-code", installed: true })];
      }
      if (command === "register_agent_tool") {
        throw new Error("config write denied");
      }
      return null;
    });

    renderAgentTools(<AgentToolCards />);

    fireEvent.click(await screen.findByText("Claude Code"));
    fireEvent.click(screen.getByRole("button", { name: "Connect Claude Code" }));

    expect(await screen.findByText("Config write denied.")).toBeInTheDocument();
    expect(screen.getByText(PLANNED_CHANGE)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connect Claude Code" })).toBeEnabled();
  });

  it("closing the modal does not register", async () => {
    mockDetect([toolStatus({ tool: "claude-code", installed: true })]);

    renderAgentTools(<AgentToolCards />);

    fireEvent.click(await screen.findByText("Claude Code"));
    expect(screen.getByText(PLANNED_CHANGE)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));

    expect(screen.queryByText(PLANNED_CHANGE)).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("register_agent_tool", expect.anything());
  });
});
