import { describe, expect, it } from "vitest";
import type { McpManualConfig, McpServerSpec } from "@/generated/ipc-bindings";
import {
  MANUAL_SETUP_EDITORS,
  buildManualSetupBlock,
  manualSetupAgentTool,
  toManualSetupEditor,
} from "./agent-tool-manual-config";

const spec: McpServerSpec = {
  command: "/usr/local/bin/node",
  args: [
    "--disable-warning=ExperimentalWarning",
    "/Users/dev/Library/Application Support/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs",
  ],
  env: { SITECMD_DB_PATH: "/Users/dev/Library/Application Support/com.sitecmd.app/sitecmd.db" },
};

function manualConfig(overrides: Partial<McpManualConfig> = {}): McpManualConfig {
  return {
    tool: "cursor",
    configPath: "~/.cursor/mcp.json",
    spec,
    snippet: JSON.stringify(
      { mcpServers: { sitecmd: { command: spec.command, args: spec.args, env: spec.env } } },
      null,
      2,
    ),
    cliCommand: null,
    ...overrides,
  };
}

describe("buildManualSetupBlock", () => {
  it("covers every editor the picker offers", () => {
    expect(MANUAL_SETUP_EDITORS).toEqual(["claude-code", "cursor", "codex", "windsurf", "generic"]);
  });

  it("renders the Claude Code CLI command exactly as the backend built it", () => {
    const cliCommand =
      "claude mcp add --scope user sitecmd --env 'SITECMD_DB_PATH=/Users/dev/Library/Application Support/com.sitecmd.app/sitecmd.db' -- /usr/local/bin/node --disable-warning=ExperimentalWarning '/Users/dev/Library/Application Support/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs'";
    const config = manualConfig({ tool: "claude-code", configPath: "~/.claude.json", cliCommand });

    const block = buildManualSetupBlock("claude-code", config);

    expect(block.language).toBe("bash");
    expect(block.location).toBe("Run in a terminal");
    expect(block.body).toBe(cliCommand);
    expect(block.note).toBeNull();
  });

  it("passes the backend snippet and config path through for Cursor and Windsurf", () => {
    const cursorConfig = manualConfig({ tool: "cursor", configPath: "~/.cursor/mcp.json" });
    const windsurfConfig = manualConfig({
      tool: "windsurf",
      configPath: "~/.codeium/windsurf/mcp_config.json",
    });

    const cursor = buildManualSetupBlock("cursor", cursorConfig);
    const windsurf = buildManualSetupBlock("windsurf", windsurfConfig);

    expect(cursor.language).toBe("json");
    expect(cursor.location).toBe("~/.cursor/mcp.json");
    expect(cursor.body).toBe(cursorConfig.snippet);
    expect(cursor.note).toContain("mcpServers");

    expect(windsurf.location).toBe("~/.codeium/windsurf/mcp_config.json");
    expect(windsurf.body).toBe(windsurfConfig.snippet);
    expect(JSON.parse(windsurf.body)).toEqual({
      mcpServers: { sitecmd: { command: spec.command, args: spec.args, env: spec.env } },
    });
  });

  it("passes the backend TOML snippet through for Codex without a merge note", () => {
    const codexSnippet = [
      "[mcp_servers.sitecmd]",
      'command = "/usr/local/bin/node"',
      "",
      "[mcp_servers.sitecmd.env]",
      'SITECMD_DB_PATH = "/Users/dev/Library/Application Support/com.sitecmd.app/sitecmd.db"',
    ].join("\n");
    const config = manualConfig({
      tool: "codex",
      configPath: "~/.codex/config.toml",
      snippet: codexSnippet,
    });

    const block = buildManualSetupBlock("codex", config);

    expect(block.language).toBe("toml");
    expect(block.location).toBe("~/.codex/config.toml");
    expect(block.body).toBe(codexSnippet);
    expect(block.note).toBeNull();
  });

  it("tells VS Code and Zed users which key their editor renames, built from spec alone", () => {
    const block = buildManualSetupBlock("generic", manualConfig());

    expect(block.label).toBe("VS Code, Cline, Zed, JetBrains");
    expect(block.note).toContain("VS Code");
    expect(block.note).toContain("context_servers");
    expect(JSON.parse(block.body)).toEqual({
      mcpServers: { sitecmd: { command: spec.command, args: spec.args, env: spec.env } },
    });
  });

  it("falls back to the generic block for unknown picker values", () => {
    expect(toManualSetupEditor("cursor")).toBe("cursor");
    expect(toManualSetupEditor("emacs")).toBe("generic");
  });

  it("maps every real editor to itself and generic to cursor for the manual config request", () => {
    expect(manualSetupAgentTool("claude-code")).toBe("claude-code");
    expect(manualSetupAgentTool("cursor")).toBe("cursor");
    expect(manualSetupAgentTool("codex")).toBe("codex");
    expect(manualSetupAgentTool("windsurf")).toBe("windsurf");
    expect(manualSetupAgentTool("generic")).toBe("cursor");
  });
});
