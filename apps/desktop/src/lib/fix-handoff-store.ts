import type { AgentTool } from "@/lib/fix-attempts";

/** Launch state; `manual` means the prompt was copied without launching a tool. */
export type HandoffPhase = "preparing" | "launching" | "opened" | "launch_failed" | "manual";

export interface FixHandoffState {
  mode: "setup" | "handoff";
  tool: AgentTool | null;
  phase: HandoffPhase;
  /** Set once create_fix_attempt resolves; lets a remount re-track the attempt. */
  attemptId: number | null;
}

/** In-flight agent handoffs keyed by issue, durable across dossier remounts. */
const states = new Map<string, FixHandoffState>();
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) listener();
}

export function fixHandoffKey(projectId: number, envUrl: string, checkId: string): string {
  return `${projectId}:${envUrl}:${checkId}`;
}

export function getFixHandoff(key: string): FixHandoffState | null {
  return states.get(key) ?? null;
}

export function setFixHandoff(key: string, state: FixHandoffState): void {
  states.set(key, state);
  emit();
}

export function patchFixHandoff(key: string, patch: Partial<FixHandoffState>): void {
  const current = states.get(key);
  if (!current) return;
  states.set(key, { ...current, ...patch });
  emit();
}

export function clearFixHandoff(key: string): void {
  if (!states.delete(key)) return;
  emit();
}

export function subscribeFixHandoff(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Test hook: handoffs persist across component lifetimes by design, so test
 *  isolation needs an explicit reset between cases. */
export function resetFixHandoffStoreForTests(): void {
  states.clear();
  listeners.clear();
}
