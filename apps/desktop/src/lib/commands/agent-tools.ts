import { command } from "./invoke";
import type { AgentTool, AgentToolStatus, McpManualConfig } from "@/generated/ipc-bindings";

export function detectAgentTools(): Promise<AgentToolStatus[]> {
  return command<AgentToolStatus[]>("detect_agent_tools");
}

export function getAgentToolManualConfig(args: { tool: AgentTool }): Promise<McpManualConfig> {
  return command<McpManualConfig>("get_agent_tool_manual_config", args);
}

export function registerAgentTool(args: { tool: AgentTool }): Promise<AgentToolStatus> {
  return command<AgentToolStatus>("register_agent_tool", args);
}

export function unregisterAgentTool(args: { tool: AgentTool }): Promise<AgentToolStatus> {
  return command<AgentToolStatus>("unregister_agent_tool", args);
}

export function launchAgentHandoff(args: {
  tool: AgentTool;
  kickoffPrompt: string;
  projectPath?: string | null;
}): Promise<void> {
  return command<void>("launch_agent_handoff", args);
}
