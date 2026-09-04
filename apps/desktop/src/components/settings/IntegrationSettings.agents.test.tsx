import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentToolStatus } from "@/lib/fix-attempts";
import { queryKeys } from "@/lib/query/query-keys";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import type { IntegrationConfig } from "./integration-services";
import { IntegrationSettings } from "./IntegrationSettings";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ success: vi.fn(), error: vi.fn(), info: vi.fn() }),
}));

const disconnected: AgentToolStatus = {
  tool: "claude-code",
  installed: true,
  registered: false,
  healthy: false,
  needsRepair: false,
  repairReason: null,
  nodeAvailable: true,
  configPath: "~/.claude.json",
  plannedChange: 'Add a "sitecmd" entry to mcpServers in ~/.claude.json',
};
const connected = { ...disconnected, registered: true, healthy: true };
const needsRepair = {
  ...connected,
  healthy: false,
  needsRepair: true,
  repairReason: "The saved SiteCMD MCP command is stale",
};
const plausible: IntegrationConfig = {
  integrationType: "plausible",
  apiKey: null,
  siteId: "example.com",
  extra: null,
  enabled: true,
};

function renderSettings(tools: AgentToolStatus[], configs: IntegrationConfig[] = []) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "detect_agent_tools") return [...tools];
    if (command === "register_agent_tool") return connected;
    if (command === "unregister_agent_tool") return disconnected;
    if (command === "fetch_integration_data") {
      return {
        integrationType: "plausible",
        data: {},
        fetchedAt: "2026-09-03T12:00:00Z",
        error: null,
      };
    }
    return [];
  });
  const queryClient = createTestQueryClient();
  render(
    <IntegrationSettings
      projectId={7}
      projectName="Example"
      url="https://example.com"
      configs={configs}
    />,
    { wrapper: withQueryClient(queryClient) },
  );
  return queryClient;
}

function section(name: string) {
  const element = screen.getByText(name).closest("section");
  if (!element) throw new Error(`No integration section for "${name}"`);
  return within(element);
}

describe("IntegrationSettings agent grouping", () => {
  beforeEach(() => invokeMock.mockReset());

  it("lists healthy agents alongside services in one Active section without duplicates", async () => {
    renderSettings(
      [connected, { ...disconnected, tool: "cursor" }, { ...needsRepair, tool: "codex" }],
      [plausible],
    );

    await screen.findByText("Claude Code");

    expect(screen.getAllByText("Active")).toHaveLength(1);
    expect(section("Active").getByRole("button", { name: /Claude Code.*Manage/ })).toBeEnabled();
    expect(
      section("Active").getByRole("button", { name: /Plausible Analytics.*Manage/ }),
    ).toBeEnabled();
    expect(section("Active").getAllByLabelText("Connected")).toHaveLength(2);
    expect(section("Agent tools").queryByText("Claude Code")).not.toBeInTheDocument();
    expect(section("Agent tools").getByRole("button", { name: /Cursor.*Connect/ })).toBeEnabled();
    expect(section("Agent tools").getByRole("button", { name: /Codex.*Repair/ })).toBeEnabled();
    expect(screen.getAllByText("Claude Code")).toHaveLength(1);
  });

  it("shows Active with only an agent connection and leaves manual setup available", async () => {
    renderSettings([connected]);

    await screen.findByText("Claude Code");

    expect(section("Active").getByRole("button", { name: /Claude Code.*Manage/ })).toBeEnabled();
    expect(section("Agent tools").getByText("Manual setup")).toBeInTheDocument();
    expect(section("Agent tools").queryByRole("button")).not.toBeInTheDocument();
  });

  it.each([
    ["connect", disconnected, "Connect"],
    ["repair", needsRepair, "Repair"],
  ] as const)("moves an agent into Active after a successful %s", async (_, status, action) => {
    renderSettings([status]);

    await screen.findByText("Claude Code");
    expect(screen.queryByText("Active")).not.toBeInTheDocument();
    fireEvent.click(
      section("Agent tools").getByRole("button", { name: new RegExp(`Claude Code.*${action}`) }),
    );
    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: `${action} Claude Code` }),
    );

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(section("Active").getByRole("button", { name: /Claude Code.*Manage/ })).toBeEnabled();
    expect(section("Agent tools").queryByText("Claude Code")).not.toBeInTheDocument();
  });

  it("returns a disconnected agent to Agent tools and removes the empty Active section", async () => {
    renderSettings([connected]);

    await screen.findByText("Claude Code");
    fireEvent.click(section("Active").getByRole("button", { name: /Claude Code.*Manage/ }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Disconnect" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.queryByText("Active")).not.toBeInTheDocument();
    expect(
      section("Agent tools").getByRole("button", { name: /Claude Code.*Connect/ }),
    ).toBeEnabled();
  });

  it("keeps a connected agent in Active when disconnect fails", async () => {
    renderSettings([connected]);
    await screen.findByText("Claude Code");
    invokeMock.mockRejectedValueOnce(new Error("config write denied"));

    fireEvent.click(section("Active").getByRole("button", { name: /Claude Code.*Manage/ }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Disconnect" }));

    expect(await screen.findByText("Config write denied.")).toBeInTheDocument();
    expect(section("Active").getByRole("button", { name: /Claude Code.*Manage/ })).toBeEnabled();
    expect(section("Agent tools").queryByText("Claude Code")).not.toBeInTheDocument();
  });

  it("regroups refreshed agent status without disturbing active services", async () => {
    const tools = [connected];
    const queryClient = renderSettings(tools, [plausible]);
    await screen.findByText("Claude Code");

    tools[0] = needsRepair;
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.settings.agentTools() });
    });

    await waitFor(() =>
      expect(section("Active").queryByText("Claude Code")).not.toBeInTheDocument(),
    );
    expect(section("Active").getByText("Plausible Analytics")).toBeInTheDocument();
    expect(
      section("Agent tools").getByRole("button", { name: /Claude Code.*Repair/ }),
    ).toBeEnabled();

    tools[0] = connected;
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.settings.agentTools() });
    });

    expect(
      await section("Active").findByRole("button", { name: /Claude Code.*Manage/ }),
    ).toBeEnabled();
    expect(section("Agent tools").queryByText("Claude Code")).not.toBeInTheDocument();
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "detect_agent_tools"),
    ).toHaveLength(3);
  });

  it("keeps active services visible while agent detection loads or fails", async () => {
    const queryClient = renderSettings([], [plausible]);
    expect(section("Active").getByText("Plausible Analytics")).toBeInTheDocument();
    expect(screen.getByLabelText("Agent tools loading state")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByLabelText("Agent tools loading state")).toBeNull());

    invokeMock.mockRejectedValueOnce(new Error("detection failed"));
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.settings.agentTools() });
    });

    expect(await section("Agent tools").findByText("Error: detection failed")).toBeInTheDocument();
    expect(section("Agent tools").getByRole("button", { name: "Retry" })).toBeEnabled();
    expect(section("Active").getByText("Plausible Analytics")).toBeInTheDocument();
  });
});
