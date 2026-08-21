import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Bot, Check, Circle, Loader2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Markdown } from "@/components/ui/markdown";
import {
  AGENT_TOOL_LABELS,
  type AgentTool,
  type AgentToolStatus,
  type FixAttempt,
} from "@/lib/fix-attempts";
import type { HandoffPhase } from "@/lib/fix-handoff-store";
import { cn } from "@/lib/utils";

interface FixWithAgentModalProps {
  mode: "setup" | "handoff";
  /** detect_agent_tools is still running for this open. */
  detecting: boolean;
  /** detect_agent_tools failed, as opposed to finding no registered tools. */
  detectFailed: boolean;
  registeredTools: AgentToolStatus[];
  selectedTool: AgentTool | null;
  creating: boolean;
  createError: string | null;
  onSelectTool: (tool: AgentTool) => void;
  onStartFix: () => void;
  onCopyBriefInstead: () => void;
  /** Handoff mode: which tool was launched and how far the loop has gotten. */
  handoffTool: AgentTool | null;
  handoffPhase: HandoffPhase;
  attempt: FixAttempt | null;
  /** The agent has not fetched the brief for a while; surface the MCP hint. */
  stuckWaiting: boolean;
  /** Web check on a remote env: verification waits for a deploy. */
  remoteWebEnv: boolean;
  onCopyKickoff: () => void;
  onTryAgain: () => void;
  onChangeTool: () => void;
  onClose: () => void;
  /** Navigate to the Integrations page; the setup button hides when omitted. */
  onOpenIntegrations?: () => void;
}

type StepState = "done" | "active" | "pending" | "failed";

/** Visible, syntax-highlighted prompt with its copy action. */
function FixPromptBlock({ prompt, onCopy }: { prompt: string; onCopy: () => void }) {
  return (
    <div className="fix-prompt-block">
      <Markdown>{["```markdown", prompt, "```"].join("\n")}</Markdown>
      <Button variant="outline" size="sm" onClick={onCopy}>
        Copy fix prompt
      </Button>
    </div>
  );
}

function stepIcon(state: StepState): ReactNode {
  switch (state) {
    case "done":
      return <Check className="icon-sm text-score-excellent" />;
    case "active":
      return <Loader2 className="spinner-sm" />;
    case "failed":
      return <X className="icon-sm text-severity-critical" />;
    case "pending":
      return <Circle className="step-pending-dot" />;
  }
}

interface HandoffStep {
  label: string;
  state: StepState;
}

/** Derive observable workflow steps from launch phase and attempt status. */
function buildSteps(
  toolLabel: string,
  phase: HandoffPhase,
  attempt: FixAttempt | null,
): HandoffStep[] {
  const briefDone = attempt !== null;
  // "manual" counts as launched: the prompt is copied and shown; the loop
  // continues the moment the user pastes it into their agent.
  const launchDone = phase === "opened" || phase === "manual";
  const pickedUp = attempt?.briefFetchedAt != null;
  const status = attempt?.status ?? "briefed";
  const reported = status === "verify_requested" || status === "verifying" || status === "verified";
  const verified = status === "verified";
  const failed = status === "verify_failed";

  return [
    { label: "Fix brief prepared", state: briefDone ? "done" : "active" },
    {
      label:
        phase === "manual"
          ? "Fix prompt copied - paste it into your agent and send it"
          : `${toolLabel} opened with the fix prompt staged`,
      state: launchDone
        ? "done"
        : phase === "launch_failed"
          ? "failed"
          : briefDone
            ? "active"
            : "pending",
    },
    {
      label: `${toolLabel} picked up the brief`,
      // The agent can report back without the pickup stamp (older MCP server
      // versions), so a later step completing also completes this one.
      state: pickedUp || reported || failed ? "done" : launchDone ? "active" : "pending",
    },
    {
      label: "Agent reported the fix",
      state: reported || failed ? "done" : pickedUp ? "active" : "pending",
    },
    {
      label: "SiteCMD re-ran the check",
      state: verified ? "done" : failed ? "failed" : reported ? "active" : "pending",
    },
  ];
}

export function FixWithAgentModal({
  mode,
  detecting,
  detectFailed,
  registeredTools,
  selectedTool,
  creating,
  createError,
  onSelectTool,
  onStartFix,
  onCopyBriefInstead,
  handoffTool,
  handoffPhase,
  attempt,
  stuckWaiting,
  remoteWebEnv,
  onCopyKickoff,
  onTryAgain,
  onChangeTool,
  onClose,
  onOpenIntegrations,
}: FixWithAgentModalProps) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [onClose]);

  const handoffLabel = handoffTool ? (AGENT_TOOL_LABELS[handoffTool] ?? handoffTool) : "your agent";
  const verified = attempt?.status === "verified";
  const failed = attempt?.status === "verify_failed";
  const steps = buildSteps(handoffLabel, handoffPhase, attempt);

  const setupBody =
    registeredTools.length > 0 ? (
      <>
        <div className="stack-snug">
          <p className="details-section-label">Send the fix to</p>
          <div className="agent-tool-list" role="radiogroup" aria-label="Agent tool">
            {registeredTools.map((tool) => (
              <Button
                key={tool.tool}
                unstyled
                type="button"
                role="radio"
                aria-checked={selectedTool === tool.tool}
                className={cn(
                  "agent-tool-row",
                  selectedTool === tool.tool && "agent-tool-row-active",
                )}
                onClick={() => onSelectTool(tool.tool)}>
                <span>{AGENT_TOOL_LABELS[tool.tool]}</span>
                {selectedTool === tool.tool ? <Check className="icon-sm" /> : null}
              </Button>
            ))}
          </div>
        </div>
        <div className="stack-snug">
          <p className="details-section-label">How it works</p>
          <ol className="agent-handoff-steps">
            <li>
              SiteCMD prepares the fix brief: the evidence, the exact files, and what passing looks
              like.
            </li>
            <li>Your agent opens with the fix prompt staged. Review it and press enter.</li>
            <li>
              When the agent reports back, SiteCMD re-runs the check itself and only marks the issue
              fixed if it passes. A fix that does not pass never counts.
            </li>
          </ol>
        </div>
      </>
    ) : (
      <>
        <p className="body-muted">
          {detectFailed
            ? "Could not check for agent tools."
            : "No agent tools are connected yet. Connect a coding agent so SiteCMD can hand fixes to it directly."}
        </p>
        <div className="button-col">
          {onOpenIntegrations ? (
            <Button variant="default" onClick={onOpenIntegrations}>
              Set up in Integrations
            </Button>
          ) : null}
          <Button variant="outline" onClick={onCopyBriefInstead} disabled={creating}>
            {creating ? <Loader2 className="spinner-sm" /> : null}
            <span>Copy the fix prompt instead</span>
          </Button>
        </div>
      </>
    );

  const handoffBody = (
    <>
      <p className="body-muted">
        Sending to {handoffLabel}
        {" · "}
        <Button unstyled className="underline-action" onClick={onChangeTool}>
          change tool
        </Button>
      </p>
      <ul className="agent-progress-steps" aria-label="Fix progress">
        {steps.map((step) => (
          <li
            key={step.label}
            className={cn("agent-progress-step", `agent-progress-step--${step.state}`)}>
            <span className="agent-progress-step-icon">{stepIcon(step.state)}</span>
            <span>{step.label}</span>
          </li>
        ))}
      </ul>
      {handoffPhase === "manual" && attempt ? (
        <div className="agent-progress-note">
          <p>Paste this fix prompt into {handoffLabel} in this project:</p>
          <FixPromptBlock prompt={attempt.kickoffPrompt} onCopy={onCopyKickoff} />
        </div>
      ) : null}
      {handoffPhase === "launch_failed" ? (
        <div className="agent-progress-note">
          <p>
            Could not open {handoffLabel}. The fix prompt is on your clipboard - open {handoffLabel}{" "}
            in this project and paste it.
          </p>
          {attempt ? (
            <FixPromptBlock prompt={attempt.kickoffPrompt} onCopy={onCopyKickoff} />
          ) : (
            <Button variant="outline" size="sm" onClick={onCopyKickoff}>
              Copy fix prompt
            </Button>
          )}
        </div>
      ) : null}
      {stuckWaiting && !verified && !failed ? (
        <div className="agent-progress-note">
          <p>
            {handoffLabel} has not picked up the brief yet. Check that the sitecmd MCP server shows
            connected (run /mcp in {handoffLabel}), or paste the fix prompt manually.
          </p>
          {attempt ? (
            <FixPromptBlock prompt={attempt.kickoffPrompt} onCopy={onCopyKickoff} />
          ) : (
            <Button variant="outline" size="sm" onClick={onCopyKickoff}>
              Copy fix prompt
            </Button>
          )}
        </div>
      ) : null}
      {remoteWebEnv &&
      !verified &&
      !failed &&
      (attempt?.status === "verify_requested" || attempt?.status === "verifying") ? (
        <div className="agent-progress-note">
          <p>
            This check verifies the live site. If your agent changed source files, deploy them -
            SiteCMD keeps re-checking and will verify automatically once the fix is live.
          </p>
        </div>
      ) : null}
      {verified ? (
        <p className="agent-progress-verified">
          Fix verified. SiteCMD re-ran the check and it passes now.
        </p>
      ) : null}
      {failed ? (
        <div className="agent-progress-note">
          <p>{attempt?.failureDetail ?? "The fix did not pass verification."}</p>
          <Button variant="outline" size="sm" onClick={onTryAgain}>
            Try again
          </Button>
        </div>
      ) : null}
      {!verified && !failed ? (
        <p className="muted-text">
          You can close this window - progress keeps tracking on the issue.
        </p>
      ) : null}
    </>
  );

  return createPortal(
    // Handoffs close only through explicit controls, never a backdrop click.
    <div className="fix-prompt-modal-backdrop" data-dossier-switch="true">
      <section
        className="fix-prompt-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="agent-handoff-title">
        <div className="fix-prompt-modal-header">
          <div className="stack-tight">
            <p className="details-section-label">Fix with your agent</p>
            <h3 id="agent-handoff-title" className="fix-prompt-modal-title">
              {mode === "handoff" ? "Your agent is on it" : "Hand this fix to your coding agent"}
            </h3>
          </div>
          <Button
            unstyled
            type="button"
            className="details-close"
            aria-label="Close agent handoff"
            onClick={onClose}>
            <X />
          </Button>
        </div>
        <div className="agent-handoff-body">
          {mode === "handoff" ? (
            handoffBody
          ) : detecting ? (
            <p className="body-muted row">
              <Loader2 className="spinner-sm" />
              <span>Checking for connected agent tools...</span>
            </p>
          ) : (
            setupBody
          )}
          {createError ? <p className="agent-handoff-error">{createError}</p> : null}
        </div>
        <div className="fix-prompt-modal-footer">
          <Button variant={verified ? "default" : "outline"} onClick={onClose} disabled={creating}>
            {verified ? "Done" : "Close"}
          </Button>
          {mode === "setup" && registeredTools.length > 0 ? (
            <Button onClick={onStartFix} disabled={creating || !selectedTool}>
              {creating ? <Loader2 className="spinner-sm" /> : <Bot className="icon-sm" />}
              <span>Start fix</span>
            </Button>
          ) : null}
        </div>
      </section>
    </div>,
    document.body,
  );
}
