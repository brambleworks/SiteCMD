import type { AgentTool, McpManualConfig, McpServerSpec } from "@/generated/ipc-bindings";
import { AGENT_TOOL_LABELS } from "@/lib/fix-attempts";

export type ManualSetupEditor = "claude-code" | "cursor" | "codex" | "windsurf" | "generic";

export interface ManualSetupBlock {
  editor: ManualSetupEditor;
  label: string;
  /** Where the block goes: a config path or "Run in a terminal". */
  location: string;
  language: "bash" | "json" | "toml";
  body: string;
  note: string | null;
}

export const MANUAL_SETUP_EDITORS: readonly ManualSetupEditor[] = [
  "claude-code",
  "cursor",
  "codex",
  "windsurf",
  "generic",
];

export const MANUAL_SETUP_EDITOR_LABELS: Record<ManualSetupEditor, string> = {
  ...AGENT_TOOL_LABELS,
  generic: "VS Code, Cline, Zed, JetBrains",
};

const MERGE_NOTE = "If the file already has an mcpServers object, add the sitecmd entry inside it.";
const GENERIC_NOTE =
  "VS Code names the list `servers` in .vscode/mcp.json and Zed names it `context_servers`; the command, args, and env values are the same.";

export function toManualSetupEditor(value: string): ManualSetupEditor {
  return (MANUAL_SETUP_EDITORS as readonly string[]).includes(value)
    ? (value as ManualSetupEditor)
    : "generic";
}

/** The AgentTool to fetch a manual config for: generic borrows Cursor's mcpServers shape. */
export function manualSetupAgentTool(editor: ManualSetupEditor): AgentTool {
  return editor === "generic" ? "cursor" : editor;
}

function mcpServersJson(spec: McpServerSpec): string {
  return JSON.stringify(
    { mcpServers: { sitecmd: { command: spec.command, args: spec.args, env: spec.env } } },
    null,
    2,
  );
}

/**
 * Format one editor's manual MCP setup block from the backend's resolved
 * config (`getAgentToolManualConfig`). Claude Code registers through its CLI,
 * so its block is the exact command the backend built. Cursor, Codex, and
 * Windsurf edit a config file, so their block is the backend's exact
 * `snippet` at its `configPath`. `generic` has no backend snippet of its
 * own; its block is a plain `mcpServers` fragment built client-side from the
 * shared launch spec so VS Code, Cline, Zed, and JetBrains users still get a
 * copyable block.
 */
export function buildManualSetupBlock(
  editor: ManualSetupEditor,
  config: McpManualConfig,
): ManualSetupBlock {
  const label = MANUAL_SETUP_EDITOR_LABELS[editor];

  if (editor === "generic") {
    return {
      editor,
      label,
      location: "Your editor's MCP settings file",
      language: "json",
      body: mcpServersJson(config.spec),
      note: GENERIC_NOTE,
    };
  }

  if (editor === "claude-code") {
    return {
      editor,
      label,
      location: "Run in a terminal",
      language: "bash",
      body: config.cliCommand ?? config.snippet,
      note: null,
    };
  }

  return {
    editor,
    label,
    location: config.configPath,
    language: editor === "codex" ? "toml" : "json",
    body: config.snippet,
    note: editor === "codex" ? null : MERGE_NOTE,
  };
}
