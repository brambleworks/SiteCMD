import {
  cancelFixAttempt as cancelFixAttemptCmd,
  createFixAttempt as createFixAttemptCmd,
  detectAgentTools as detectAgentToolsCmd,
  getFixAttemptForIssue as getFixAttemptForIssueCmd,
  launchAgentHandoff as launchAgentHandoffCmd,
  registerAgentTool as registerAgentToolCmd,
  unregisterAgentTool as unregisterAgentToolCmd,
} from "@/lib/commands";
import type { CreateFixAttemptArgs } from "@/generated/ipc-bindings";

// Must match the AgentTool serde enum in src-tauri/src/core/agent_tools.rs
export type AgentTool = "claude-code" | "codex" | "cursor" | "windsurf";

// Must match ALL_FIX_ATTEMPT_STATUSES in src-tauri/src/db/fix_attempts.rs
export type FixAttemptStatus =
  | "briefed"
  | "verify_requested"
  | "verifying"
  | "verified"
  | "verify_failed"
  | "canceled"
  | "expired";

export interface FixAttempt {
  id: number;
  status: FixAttemptStatus;
  agentTool: AgentTool;
  agentSummary: string | null;
  failureDetail: string | null;
  kickoffPrompt: string;
  /** Stamped by the MCP server the first time the agent fetches the brief. */
  briefFetchedAt: number | null;
  createdAt: number;
  updatedAt: number;
}

export interface BriefLocation {
  label: string;
  path: string;
  line: number | null;
  reason: string;
}

export interface AgentToolStatus {
  tool: AgentTool;
  installed: boolean;
  registered: boolean;
  healthy: boolean;
  needsRepair: boolean;
  repairReason: string | null;
  nodeAvailable: boolean;
  configPath: string;
  plannedChange: string;
}

export interface CreateFixAttemptInput {
  projectId: number;
  envUrl: string;
  checkId: string;
  agentTool: AgentTool;
  title: string;
  severity: string;
  description: string;
  url: string;
  whyItMatters?: string | null;
  evidence?: unknown;
  manualFix?: string | null;
  detectedStack?: unknown;
  codeLocations?: BriefLocation[];
  previousFailure?: string | null;
}

// Must match ACTIVE_FIX_ATTEMPT_STATUSES in src-tauri/src/db/fix_attempts.rs
export const ACTIVE_ATTEMPT_STATUSES: readonly FixAttemptStatus[] = [
  "briefed",
  "verify_requested",
  "verifying",
];

export const AGENT_TOOL_LABELS: Record<AgentTool, string> = {
  "claude-code": "Claude Code",
  codex: "Codex",
  cursor: "Cursor",
  windsurf: "Windsurf",
};

export function isAttemptActive(status: FixAttemptStatus): boolean {
  return ACTIVE_ATTEMPT_STATUSES.includes(status);
}

export function createFixAttempt(input: CreateFixAttemptInput): Promise<FixAttempt> {
  // FixAttempt narrows the wire DTO's string status/agentTool to their unions;
  // the wire only ever returns valid members, so this boundary cast is safe.
  return createFixAttemptCmd({ args: input as CreateFixAttemptArgs }) as Promise<FixAttempt>;
}

export function getFixAttemptForIssue(
  projectId: number,
  envUrl: string,
  checkId: string,
  title: string,
): Promise<FixAttempt | null> {
  return getFixAttemptForIssueCmd({
    projectId,
    envUrl,
    checkId,
    title,
  }) as Promise<FixAttempt | null>;
}

/** Whether a web fix targets a remote environment and must await deployment. */
export function isRemoteWebAttempt(checkId: string, envUrl: string): boolean {
  if (checkId.startsWith("code_scan.")) return false;
  try {
    const host = new URL(envUrl).hostname.toLowerCase();
    const local =
      host === "localhost" ||
      host === "0.0.0.0" ||
      host === "::1" ||
      host === "[::1]" ||
      host.endsWith(".localhost") ||
      host.endsWith(".local") ||
      host.startsWith("127.");
    return !local;
  } catch {
    return false;
  }
}

/** Opens an agent deep link with a staged prompt; rejects if no handler exists. */
export function launchAgentHandoff(
  tool: AgentTool,
  kickoffPrompt: string,
  projectPath?: string | null,
): Promise<void> {
  return launchAgentHandoffCmd({
    tool,
    kickoffPrompt,
    projectPath: projectPath ?? null,
  });
}

export function cancelFixAttempt(attemptId: number): Promise<void> {
  return cancelFixAttemptCmd({ attemptId });
}

export function detectAgentTools(): Promise<AgentToolStatus[]> {
  return detectAgentToolsCmd();
}

export function registerAgentTool(tool: AgentTool): Promise<AgentToolStatus> {
  return registerAgentToolCmd({ tool });
}

export function unregisterAgentTool(tool: AgentTool): Promise<AgentToolStatus> {
  return unregisterAgentToolCmd({ tool });
}
