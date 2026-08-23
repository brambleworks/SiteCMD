import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, copyToClipboardMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  copyToClipboardMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/lib/clipboard", () => ({
  copyToClipboard: (text: string) => copyToClipboardMock(text),
}));

import { AgentToolManualSetup } from "./AgentToolManualSetup";
import type { AgentTool, McpManualConfig } from "@/generated/ipc-bindings";
import { withQueryClient } from "@/test-utils/query-client";

const spec = {
  command: "/usr/local/bin/node",
  args: ["--disable-warning=ExperimentalWarning", "/opt/sitecmd-mcp/sitecmd-mcp.mjs"],
  env: { SITECMD_DB_PATH: "/opt/sitecmd/sitecmd.db" },
};

const CLAUDE_CLI_COMMAND =
  "claude mcp add --scope user sitecmd --env 'SITECMD_DB_PATH=/opt/sitecmd/sitecmd.db' -- /usr/local/bin/node --disable-warning=ExperimentalWarning /opt/sitecmd-mcp/sitecmd-mcp.mjs";

const CONFIG_PATHS: Record<AgentTool, string> = {
  "claude-code": "~/.claude.json",
  cursor: "~/.cursor/mcp.json",
  codex: "~/.codex/config.toml",
  windsurf: "~/.codeium/windsurf/mcp_config.json",
};

function configFor(tool: AgentTool): McpManualConfig {
  return {
    tool,
    configPath: CONFIG_PATHS[tool],
    spec,
    snippet:
      tool === "codex"
        ? '[mcp_servers.sitecmd]\ncommand = "/usr/local/bin/node"'
        : JSON.stringify({ mcpServers: { sitecmd: spec } }, null, 2),
    cliCommand: tool === "claude-code" ? CLAUDE_CLI_COMMAND : null,
  };
}

describe("AgentToolManualSetup", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    copyToClipboardMock.mockReset().mockResolvedValue(true);
    invokeMock.mockImplementation(async (command: string, args?: { tool: AgentTool }) =>
      command === "get_agent_tool_manual_config" && args ? configFor(args.tool) : null,
    );
  });

  it("stays closed and quiet until opened, then requests Claude Code's config first", async () => {
    render(<AgentToolManualSetup />, { wrapper: withQueryClient() });

    expect(screen.getByText("Manual setup")).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("Manual setup"));

    // The Claude Code block shows the backend's exact CLI command.
    expect(await screen.findByText(CLAUDE_CLI_COMMAND)).toBeInTheDocument();
    expect(screen.getByText("Run in a terminal")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("get_agent_tool_manual_config", {
      tool: "claude-code",
    });
  });

  it("switches to Windsurf, requests its config, and copies the backend snippet", async () => {
    render(<AgentToolManualSetup />, { wrapper: withQueryClient() });
    fireEvent.click(screen.getByText("Manual setup"));
    await screen.findByText(CLAUDE_CLI_COMMAND);

    fireEvent.change(screen.getByLabelText("Editor"), { target: { value: "windsurf" } });

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_agent_tool_manual_config", {
        tool: "windsurf",
      }),
    );
    expect(await screen.findByText("~/.codeium/windsurf/mcp_config.json")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Copy Windsurf setup" }));
    await waitFor(() => expect(copyToClipboardMock).toHaveBeenCalledTimes(1));
    expect(copyToClipboardMock).toHaveBeenCalledWith(configFor("windsurf").snippet);
  });

  it("requests Cursor's config for the generic editor and builds mcpServers JSON from spec", async () => {
    render(<AgentToolManualSetup />, { wrapper: withQueryClient() });
    fireEvent.click(screen.getByText("Manual setup"));
    await screen.findByText(CLAUDE_CLI_COMMAND);

    fireEvent.change(screen.getByLabelText("Editor"), { target: { value: "generic" } });

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_agent_tool_manual_config", { tool: "cursor" }),
    );
    fireEvent.click(
      await screen.findByRole("button", { name: "Copy VS Code, Cline, Zed, JetBrains setup" }),
    );
    await waitFor(() => expect(copyToClipboardMock).toHaveBeenCalledTimes(1));
    expect(JSON.parse(copyToClipboardMock.mock.calls[0][0])).toEqual({
      mcpServers: { sitecmd: spec },
    });
  });

  it("explains a failed spec instead of hiding the disclosure", async () => {
    invokeMock.mockImplementation(async () => {
      throw "could not resolve the SiteCMD database path";
    });
    render(<AgentToolManualSetup />, { wrapper: withQueryClient() });
    fireEvent.click(screen.getByText("Manual setup"));

    expect(
      await screen.findByText(/could not resolve the SiteCMD database path/),
    ).toBeInTheDocument();
  });
});
