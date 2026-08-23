import {
  useCallback,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { Bot } from "lucide-react";
import { FixWithAgentModal } from "@/components/issues/FixWithAgentModal";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { copyToClipboard } from "@/lib/clipboard";
import { openPathInEditor } from "@/lib/desktop-actions";
import {
  AGENT_TOOL_LABELS,
  createFixAttempt,
  detectAgentTools,
  getFixAttemptForIssue,
  hasPromptDeepLink,
  isRemoteWebAttempt,
  launchAgentHandoff,
  type AgentTool,
  type AgentToolStatus,
  type BriefLocation,
  type FixAttempt,
} from "@/lib/fix-attempts";
import {
  clearFixHandoff,
  fixHandoffKey,
  getFixHandoff,
  patchFixHandoff,
  setFixHandoff,
  subscribeFixHandoff,
} from "@/lib/fix-handoff-store";
import { useTauriEvent } from "@/hooks/useTauriEvent";

const AGENT_TOOL_STORAGE_KEY = "sitecmd:agent-tool";
// The kickoff prompt lets this fallback fetch its brief without registration.
const FALLBACK_AGENT_TOOL: AgentTool = "claude-code";
const STUCK_WAITING_MS = 60_000;
// Poll for MCP-written progress that the watcher does not emit.
const PROGRESS_POLL_MS = 3_000;

interface FixWithAgentActionProps {
  projectId: number;
  envUrl: string;
  checkId: string;
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
  /** Increment to dispatch another retry. */
  openSignal?: number;
  /** Absolute project root for editor handoff. */
  projectPath?: string | null;
  onAttemptCreated?: (attempt: FixAttempt) => void;
  /** Navigate to Integrations when setup is available. */
  onOpenIntegrations?: () => void;
}

function readRememberedAgentTool(): AgentTool | null {
  try {
    const value = window.localStorage.getItem(AGENT_TOOL_STORAGE_KEY);
    return value !== null && value in { "claude-code": 1, codex: 1, cursor: 1 }
      ? (value as AgentTool)
      : null;
  } catch {
    return null;
  }
}

function persistRememberedAgentTool(tool: AgentTool): void {
  try {
    window.localStorage.setItem(AGENT_TOOL_STORAGE_KEY, tool);
  } catch {
    // Remembering the choice is best-effort only.
  }
}

export function FixWithAgentAction({
  projectId,
  envUrl,
  checkId,
  title,
  severity,
  description,
  url,
  whyItMatters,
  evidence,
  manualFix,
  detectedStack,
  codeLocations,
  previousFailure,
  openSignal,
  projectPath,
  onAttemptCreated,
  onOpenIntegrations,
}: FixWithAgentActionProps) {
  const { success, error: toastError } = useToast();
  // The module store keeps the modal alive across dashboard remounts.
  const handoffStoreKey = fixHandoffKey(projectId, envUrl, checkId);
  const handoff = useSyncExternalStore(subscribeFixHandoff, () => getFixHandoff(handoffStoreKey));
  // null while detect_agent_tools is still running for this open.
  const [tools, setTools] = useState<AgentToolStatus[] | null>(null);
  const [detectFailed, setDetectFailed] = useState(false);
  const [selectedTool, setSelectedTool] = useState<AgentTool | null>(null);
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [rememberedTool, setRememberedTool] = useState<AgentTool | null>(() =>
    readRememberedAgentTool(),
  );
  // Rehydrated from the stored attempt id after a remount.
  const [liveAttempt, setLiveAttempt] = useState<FixAttempt | null>(null);
  const [stuckWaiting, setStuckWaiting] = useState(false);
  const disposedRef = useRef(false);
  useEffect(() => {
    disposedRef.current = false;
    return () => {
      disposedRef.current = true;
    };
  }, []);

  // Detected agents each receive a dedicated action button.
  const [connectedAgents, setConnectedAgents] = useState<AgentToolStatus[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const detected = await detectAgentTools();
        if (!cancelled) setConnectedAgents(detected);
      } catch {
        if (!cancelled) setConnectedAgents([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Events and polling cover watcher and MCP progress, including remount rehydration.
  const trackedAttemptId = handoff?.mode === "handoff" ? handoff.attemptId : null;
  const attemptSettled =
    liveAttempt !== null &&
    liveAttempt.id === trackedAttemptId &&
    liveAttempt.status !== "briefed" &&
    liveAttempt.status !== "verify_requested" &&
    liveAttempt.status !== "verifying";
  const progressActive = trackedAttemptId !== null && !attemptSettled;
  const refetchTrackedAttempt = useCallback(async () => {
    if (trackedAttemptId === null) return;
    try {
      const next = await getFixAttemptForIssue(projectId, envUrl, checkId, title);
      if (!disposedRef.current && next && next.id === trackedAttemptId) setLiveAttempt(next);
    } catch {
      // The next poll retries.
    }
  }, [trackedAttemptId, projectId, envUrl, checkId, title]);

  useEffect(() => {
    if (!progressActive) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- starts progress polling: an immediate refetch plus an interval while the attempt is active
    void refetchTrackedAttempt();
    const interval = window.setInterval(() => void refetchTrackedAttempt(), PROGRESS_POLL_MS);
    return () => window.clearInterval(interval);
  }, [progressActive, refetchTrackedAttempt]);
  useTauriEvent(
    "fix-attempt-updated",
    () => {
      void refetchTrackedAttempt();
    },
    { enabled: progressActive },
  );

  // Surface setup help when an opened agent never fetches the brief.
  const waitingForPickup =
    handoff?.mode === "handoff" &&
    handoff.phase === "opened" &&
    liveAttempt !== null &&
    liveAttempt.id === handoff.attemptId &&
    liveAttempt.briefFetchedAt === null &&
    liveAttempt.status === "briefed";
  useEffect(() => {
    if (!waitingForPickup) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- resets the stuck-waiting flag when pickup-waiting ends; paired with the setTimeout below
      setStuckWaiting(false);
      return;
    }
    const timer = window.setTimeout(() => setStuckWaiting(true), STUCK_WAITING_MS);
    return () => window.clearTimeout(timer);
  }, [waitingForPickup]);

  const createAndCopy = async (tool: AgentTool): Promise<FixAttempt | null> => {
    setCreating(true);
    setCreateError(null);
    try {
      const attempt = await createFixAttempt({
        projectId,
        envUrl,
        checkId,
        agentTool: tool,
        title,
        severity,
        description,
        url,
        whyItMatters,
        evidence,
        manualFix,
        detectedStack,
        codeLocations,
        previousFailure:
          previousFailure ??
          (liveAttempt?.status === "verify_failed" ? liveAttempt.failureDetail : null),
      });
      // Keep a clipboard fallback for failed deep links and manual pasting.
      await copyToClipboard(attempt.kickoffPrompt);
      return attempt;
    } catch (err) {
      const message = String(err);
      setCreateError(message);
      toastError("Could not start the fix", message);
      return null;
    } finally {
      setCreating(false);
    }
  };

  // Create the attempt, stage its prompt, and open the selected agent.
  const dispatch = async (tool: AgentTool) => {
    setFixHandoff(handoffStoreKey, {
      mode: "handoff",
      tool,
      phase: "preparing",
      attemptId: null,
    });
    setLiveAttempt(null);
    setStuckWaiting(false);

    const attempt = await createAndCopy(tool);
    if (!attempt) return; // createError renders in the modal
    setLiveAttempt(attempt);
    patchFixHandoff(handoffStoreKey, { attemptId: attempt.id });
    persistRememberedAgentTool(tool);
    setRememberedTool(tool);
    onAttemptCreated?.(attempt);

    if (!hasPromptDeepLink(tool)) {
      // No deep link exists to open, so the copied prompt is the finished handoff, not a failure.
      patchFixHandoff(handoffStoreKey, { phase: "manual" });
      return;
    }
    patchFixHandoff(handoffStoreKey, { phase: "launching" });
    if (tool === "cursor" && projectPath) {
      try {
        await openPathInEditor(projectPath);
      } catch {
        // Continue with the prompt deep link.
      }
    }
    try {
      await launchAgentHandoff(tool, attempt.kickoffPrompt, projectPath);
      patchFixHandoff(handoffStoreKey, { phase: "opened" });
    } catch {
      // The clipboard fallback is already ready for manual pasting.
      patchFixHandoff(handoffStoreKey, { phase: "launch_failed" });
    }
  };

  const runDetection = useCallback(async () => {
    setTools(null);
    setDetectFailed(false);
    setSelectedTool(null);
    let detected: AgentToolStatus[] = [];
    try {
      detected = await detectAgentTools();
    } catch {
      setDetectFailed(true);
    }
    const registered = detected.filter((tool) => tool.healthy);
    const remembered = readRememberedAgentTool();
    setSelectedTool(
      registered.find((tool) => tool.tool === remembered)?.tool ?? registered[0]?.tool ?? null,
    );
    setTools(detected);
  }, []);

  // The effect below owns detection, including after remount rehydration.
  const detectRanRef = useRef(false);
  const handleOpenSetup = useCallback(() => {
    setFixHandoff(handoffStoreKey, {
      mode: "setup",
      tool: null,
      phase: "preparing",
      attemptId: getFixHandoff(handoffStoreKey)?.attemptId ?? null,
    });
    setCreateError(null);
    detectRanRef.current = false;
    setTools(null);
    setDetectFailed(false);
    setSelectedTool(null);
  }, [handoffStoreKey]);

  useEffect(() => {
    if (handoff?.mode !== "setup") {
      detectRanRef.current = false;
      return;
    }
    if (tools !== null || detectRanRef.current) return;
    detectRanRef.current = true;
    void runDetection();
  }, [handoff?.mode, tools, runDetection]);

  const dispatchOrOpen = async () => {
    if (creating) return;
    if (!rememberedTool) {
      void handleOpenSetup();
      return;
    }
    setFixHandoff(handoffStoreKey, {
      mode: "handoff",
      tool: rememberedTool,
      phase: "preparing",
      attemptId: null,
    });
    setLiveAttempt(null);
    setStuckWaiting(false);
    let detected: AgentToolStatus[] = [];
    let detectionFailed = false;
    try {
      detected = await detectAgentTools();
    } catch {
      detectionFailed = true;
    }
    const registered = detected.filter((tool) => tool.healthy);
    const fastTool = registered.find((tool) => tool.tool === rememberedTool)?.tool ?? null;
    if (!fastTool) {
      // Seed setup with the completed detection result.
      setFixHandoff(handoffStoreKey, {
        mode: "setup",
        tool: null,
        phase: "preparing",
        attemptId: null,
      });
      setDetectFailed(detectionFailed);
      setSelectedTool(registered[0]?.tool ?? null);
      setTools(detected);
      return;
    }
    await dispatch(fastTool);
  };
  // Keep retry dispatch current without making the effect depend on a render-local function.
  const dispatchFromRetrySignal = useEffectEvent(() => {
    void dispatchOrOpen();
  });

  // Only a post-mount signal increase dispatches a retry.
  const lastSignalRef = useRef(openSignal ?? 0);
  useEffect(() => {
    const signal = openSignal ?? 0;
    if (signal <= lastSignalRef.current) return;
    lastSignalRef.current = signal;
    dispatchFromRetrySignal();
  }, [openSignal]);

  const registeredTools = tools?.filter((tool) => tool.healthy) ?? [];
  const registeredAgents = (connectedAgents ?? []).filter((tool) => tool.healthy);

  const handleStartFix = async () => {
    if (!selectedTool || creating) return;
    await dispatch(selectedTool);
  };

  const handleCopyBriefInstead = async () => {
    if (creating) return;
    const attempt = await createAndCopy(FALLBACK_AGENT_TOOL);
    if (!attempt) return;
    // Keep tracking after the user manually pastes the prompt.
    setLiveAttempt(attempt);
    setFixHandoff(handoffStoreKey, {
      mode: "handoff",
      tool: null,
      phase: "manual",
      attemptId: attempt.id,
    });
    onAttemptCreated?.(attempt);
  };

  const handleCopyKickoff = async () => {
    if (!liveAttempt) return;
    try {
      await copyToClipboard(liveAttempt.kickoffPrompt);
      success("Fix prompt copied", "Paste it into your agent in this project.");
    } catch (err) {
      toastError("Could not copy the fix prompt", String(err));
    }
  };

  const modalAttempt =
    liveAttempt !== null && liveAttempt.id === (handoff?.attemptId ?? null) ? liveAttempt : null;

  return (
    <>
      {registeredAgents.length > 0 ? (
        <div className="dossier-rail-button-stack">
          {registeredAgents.map((agent) => (
            <Button
              key={agent.tool}
              variant="default"
              onClick={() => void dispatch(agent.tool)}
              disabled={creating}
              aria-label={`Fix with ${AGENT_TOOL_LABELS[agent.tool]}`}>
              <Bot className="icon-sm" />
              <span>Fix with {AGENT_TOOL_LABELS[agent.tool]}</span>
            </Button>
          ))}
        </div>
      ) : (
        <div className="dossier-rail-button-stack">
          <Button
            variant="default"
            onClick={() => void dispatchOrOpen()}
            disabled={creating}
            aria-label="Fix with your agent">
            <Bot className="icon-sm" />
            <span>Fix with your agent</span>
          </Button>
        </div>
      )}
      {handoff !== null ? (
        <FixWithAgentModal
          mode={handoff.mode}
          detecting={tools === null}
          detectFailed={detectFailed}
          registeredTools={registeredTools}
          selectedTool={selectedTool}
          creating={creating}
          createError={createError}
          onSelectTool={setSelectedTool}
          onStartFix={() => void handleStartFix()}
          onCopyBriefInstead={() => void handleCopyBriefInstead()}
          handoffTool={handoff.tool}
          handoffPhase={handoff.phase}
          attempt={modalAttempt}
          stuckWaiting={stuckWaiting}
          remoteWebEnv={isRemoteWebAttempt(checkId, envUrl)}
          onCopyKickoff={() => void handleCopyKickoff()}
          onTryAgain={() => {
            if (handoff.tool) void dispatch(handoff.tool);
          }}
          onChangeTool={() => void handleOpenSetup()}
          onClose={() => clearFixHandoff(handoffStoreKey)}
          onOpenIntegrations={
            onOpenIntegrations
              ? () => {
                  onOpenIntegrations();
                  clearFixHandoff(handoffStoreKey);
                }
              : undefined
          }
        />
      ) : null}
    </>
  );
}
