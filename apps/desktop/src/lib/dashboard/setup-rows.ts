import type { BootstrapTask, SetupRow } from "./types";

const MAX_ROWS = 5;

/** Returns prioritized, capped setup tasks for the dashboard. */
export function buildSetupRows(
  bootstrap: BootstrapTask[],
  onOpenBootstrap?: (task: BootstrapTask) => void,
): SetupRow[] {
  return [...bootstrap]
    .sort((a, b) => a.priority - b.priority)
    .slice(0, MAX_ROWS)
    .map((b) => ({
      id: `bootstrap:${b.kind}`,
      label: b.label,
      value: b.value,
      onOpen: () => onOpenBootstrap?.(b),
    }));
}
