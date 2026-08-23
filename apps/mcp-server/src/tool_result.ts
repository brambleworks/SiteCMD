/** Shared tool-result helpers and annotation constants for every registered MCP tool. */

export type ToolResult = {
  content: Array<{ type: "text"; text: string }>;
  isError?: boolean;
};

export function text(value: string): ToolResult {
  return { content: [{ type: "text", text: value }] };
}

/** One error path for every tool: failures reach the agent as isError results, never transport faults. */
export function runTool(body: () => ToolResult): ToolResult {
  try {
    return body();
  } catch (error) {
    return {
      content: [
        { type: "text", text: `Error: ${error instanceof Error ? error.message : String(error)}` },
      ],
      isError: true,
    };
  }
}

export const READ_ONLY = {
  readOnlyHint: true,
  destructiveHint: false,
  idempotentHint: true,
  openWorldHint: false,
} as const;

/** Guarded updates to existing rows; repeating the call converges on the same row state. */
export const WRITES_LOCAL_DB = {
  readOnlyHint: false,
  destructiveHint: false,
  idempotentHint: true,
  openWorldHint: false,
} as const;
