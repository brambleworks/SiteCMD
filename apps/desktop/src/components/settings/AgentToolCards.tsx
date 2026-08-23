import { useState, type ComponentType } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useToast } from "@/hooks/useToast";
import { Button } from "@/components/ui/button";
import { ClaudeLogo, CodexLogo, CursorLogo, WindsurfLogo } from "@/components/icons/BrandLogos";
import { IntegrationRow } from "./IntegrationRow";
import { IntegrationModal } from "./IntegrationModal";
import { AgentToolManualSetup } from "./AgentToolManualSetup";
import {
  AGENT_TOOL_LABELS,
  detectAgentTools,
  registerAgentTool,
  unregisterAgentTool,
  type AgentTool,
  type AgentToolStatus,
} from "@/lib/fix-attempts";
import { queryKeys } from "@/lib/query/query-keys";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { useVisibilityRefresh } from "@/lib/useVisibilityRefresh";

const AGENT_TOOL_LOGOS: Record<AgentTool, ComponentType<{ className?: string }>> = {
  "claude-code": ClaudeLogo,
  codex: CodexLogo,
  cursor: CursorLogo,
  windsurf: WindsurfLogo,
};

const AGENT_TOOL_VISIBILITY_STALE_MS = 30_000;

function AgentToolBadge({ tool }: { tool: AgentTool }) {
  const Logo = AGENT_TOOL_LOGOS[tool];
  return (
    <div className="agent-tool-badge">
      <Logo className="icon-lg text-foreground" />
    </div>
  );
}

export function AgentToolCards() {
  const toast = useToast();
  const queryClient = useQueryClient();
  const queryKey = queryKeys.settings.agentTools();
  const toolsQuery = useQuery<AgentToolStatus[]>({
    queryKey,
    queryFn: async () => (await detectAgentTools()) ?? [],
    // Keep cached rows visible while checking for external tool changes.
    refetchOnMount: "always",
  });
  useVisibilityRefresh({
    staleAfterMs: AGENT_TOOL_VISIBILITY_STALE_MS,
    onRefresh: () => void toolsQuery.refetch(),
  });
  const tools = toolsQuery.data ?? [];
  const loading = toolsQuery.isPending;
  const detectError = toolsQuery.isError ? String(toolsQuery.error) : null;
  const [modalTool, setModalTool] = useState<AgentTool | null>(null);
  const [busyTools, setBusyTools] = useState<Partial<Record<AgentTool, boolean>>>({});
  const [cardErrors, setCardErrors] = useState<Partial<Record<AgentTool, string>>>({});

  const setToolBusy = (tool: AgentTool, busy: boolean) => {
    setBusyTools((prev) => ({ ...prev, [tool]: busy }));
  };

  const clearCardError = (tool: AgentTool) => {
    setCardErrors((prev) => {
      const next = { ...prev };
      delete next[tool];
      return next;
    });
  };

  const replaceTool = (next: AgentToolStatus) => {
    queryClient.setQueryData<AgentToolStatus[]>(queryKey, (current = []) =>
      current.map((item) => (item.tool === next.tool ? next : item)),
    );
  };

  const handleConfirmConnect = async (tool: AgentTool) => {
    const wasRepair = tools.find((item) => item.tool === tool)?.needsRepair ?? false;
    setToolBusy(tool, true);
    clearCardError(tool);
    try {
      replaceTool(await registerAgentTool(tool));
      setModalTool(null);
      toast.success(`${AGENT_TOOL_LABELS[tool]} ${wasRepair ? "repaired" : "connected"}`);
    } catch (error) {
      setCardErrors((prev) => ({ ...prev, [tool]: String(error) }));
    } finally {
      setToolBusy(tool, false);
    }
  };

  const handleDisconnect = async (tool: AgentTool) => {
    setToolBusy(tool, true);
    clearCardError(tool);
    try {
      replaceTool(await unregisterAgentTool(tool));
      setModalTool(null);
      toast.info(`${AGENT_TOOL_LABELS[tool]} disconnected`);
    } catch (error) {
      setCardErrors((prev) => ({ ...prev, [tool]: String(error) }));
    } finally {
      setToolBusy(tool, false);
    }
  };

  const modalItem = modalTool ? (tools.find((item) => item.tool === modalTool) ?? null) : null;
  const nodeMissing = tools.some((item) => item.installed && !item.nodeAvailable);

  const renderModalBody = (item: AgentToolStatus) => {
    const label = AGENT_TOOL_LABELS[item.tool];
    const busy = Boolean(busyTools[item.tool]);
    const cardError = cardErrors[item.tool];

    if (item.healthy) {
      return (
        <div className="subtle-divider-top integration-modal-body">
          <p className="text-13-muted text-relaxed">
            {label} is connected. SiteCMD hands it issues to fix and reads its reports for
            verification.
          </p>
          <Button
            variant="outline"
            className="btn--block integration-disconnect-btn"
            disabled={busy}
            onClick={() => void handleDisconnect(item.tool)}>
            {busy ? "Disconnecting..." : "Disconnect"}
          </Button>
          {cardError ? <p className="agent-handoff-error">{cardError}</p> : null}
        </div>
      );
    }

    if (item.needsRepair) {
      return (
        <div className="subtle-divider-top integration-modal-body">
          <p className="text-13-muted text-relaxed">
            {item.repairReason ?? `${label}'s SiteCMD connection is stale or cannot start.`}
          </p>
          <p className="text-13-muted text-relaxed">
            SiteCMD will replace only its MCP entry with the current command and verify that the
            server can open the local database.
          </p>
          <div className="black-code-panel">
            <code className="compact-code-block">{item.plannedChange}</code>
          </div>
          <div className="stack-tight">
            <Button
              className="btn--block"
              disabled={busy || !item.installed || !item.nodeAvailable}
              onClick={() => void handleConfirmConnect(item.tool)}>
              {busy ? "Repairing..." : `Repair ${label}`}
            </Button>
            <Button
              variant="outline"
              className="btn--block integration-disconnect-btn"
              disabled={busy}
              onClick={() => void handleDisconnect(item.tool)}>
              Disconnect stale entry
            </Button>
          </div>
          {cardError ? <p className="agent-handoff-error">{cardError}</p> : null}
        </div>
      );
    }

    if (!item.installed) {
      return (
        <div className="subtle-divider-top integration-modal-body">
          <p className="text-13-muted text-relaxed">
            SiteCMD could not find {label} on this computer. Install it and come back, or use Manual
            setup under the agent tool list to paste the config yourself.
          </p>
        </div>
      );
    }

    return (
      <div className="subtle-divider-top integration-modal-body">
        <p className="text-13-muted text-relaxed">
          SiteCMD will add its MCP server to {label} with this change:
        </p>
        <div className="black-code-panel">
          <code className="compact-code-block">{item.plannedChange}</code>
        </div>
        <p className="text-13-muted text-relaxed">
          SiteCMD never edits this file without this confirmation.
        </p>
        <Button
          className="btn--block"
          disabled={busy || !item.nodeAvailable}
          onClick={() => void handleConfirmConnect(item.tool)}>
          {busy ? "Connecting..." : `Connect ${label}`}
        </Button>
        {cardError ? <p className="agent-handoff-error">{cardError}</p> : null}
      </div>
    );
  };

  return (
    <>
      <section className="stack-base">
        <div className="stack-tight">
          <p className="row-title-md">Agent tools</p>
          <p className="text-13-muted text-relaxed">
            Connect the AI tool you already use so it can fix SiteCMD issues and report back.
          </p>
        </div>

        {loading ? (
          <LoadingRegion label="Agent tools loading state" className="integration-section-list">
            {[0, 1, 2].map((index) => (
              <div key={index} className="list-row">
                <Skeleton className="agent-skeleton-icon" />
                <Skeleton className="agent-skeleton-name" />
                <Skeleton className="agent-skeleton-action" />
              </div>
            ))}
          </LoadingRegion>
        ) : detectError ? (
          <div className="row-loose">
            <p className="agent-handoff-error">{detectError}</p>
            <Button variant="outline" size="sm" onClick={() => void toolsQuery.refetch()}>
              Retry
            </Button>
          </div>
        ) : (
          <>
            {nodeMissing ? (
              <div className="danger-callout-row">
                <p className="text-body text-relaxed">
                  These connections run SiteCMD&apos;s agent connector with Node.js, which needs
                  Node 22.22.1 or newer on your PATH. Install or update Node, then try again.
                </p>
              </div>
            ) : null}
            <div className="integration-section-list">
              {tools.map((item) => (
                <IntegrationRow
                  key={item.tool}
                  dataIntegration={item.tool}
                  icon={<AgentToolBadge tool={item.tool} />}
                  name={AGENT_TOOL_LABELS[item.tool]}
                  connected={item.healthy}
                  actionLabel={item.needsRepair ? "Repair" : item.healthy ? "Manage" : "Connect"}
                  disabled={
                    Boolean(busyTools[item.tool]) ||
                    (!item.registered && (!item.installed || !item.nodeAvailable))
                  }
                  onOpen={() => {
                    clearCardError(item.tool);
                    setModalTool(item.tool);
                  }}
                />
              ))}
            </div>
          </>
        )}
        <AgentToolManualSetup />
      </section>

      {modalItem ? (
        <IntegrationModal
          title={AGENT_TOOL_LABELS[modalItem.tool]}
          icon={<AgentToolBadge tool={modalItem.tool} />}
          onClose={() => setModalTool(null)}>
          {renderModalBody(modalItem)}
        </IntegrationModal>
      ) : null}
    </>
  );
}
