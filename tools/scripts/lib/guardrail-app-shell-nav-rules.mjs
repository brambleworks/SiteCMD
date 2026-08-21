const NAV_STATE_MODULES = new Set([
  "apps/desktop/src/app/useNavigationState.ts",
  "apps/desktop/src/app/useAppTargetNavigation.ts",
]);

const SELECTION_CONSUMER_MODULES = new Set(["apps/desktop/src/hooks/useAppShellOrchestration.ts"]);

// `someRef.current = activeProject...` / `= activeEnv...` - a selection mirror.
const SELECTION_MIRROR_RE = /\.current\s*=\s*active(?:Project|Env)\b/;

// `Dispatch<SetStateAction<...>>` - the setter-shim signature (whitespace-tolerant).
const SETTER_SHIM_RE = /Dispatch\s*<\s*SetStateAction\s*</;
// The generic escape hatch: an `APPLY` action or an `apply:` updater payload.
const APPLY_ESCAPE_RE = /type:\s*["']APPLY["']|\bapply:\s*\(/;

function isCommentLine(trimmed) {
  return trimmed.startsWith("//") || trimmed.startsWith("*");
}

export function appShellNavFailures(read, sourceFiles) {
  const failures = [];
  for (const file of sourceFiles) {
    const isNavModule = NAV_STATE_MODULES.has(file);
    const isSelectionConsumer = SELECTION_CONSUMER_MODULES.has(file);
    if (!isNavModule && !isSelectionConsumer) continue;

    const lines = read(file).split("\n");
    for (let i = 0; i < lines.length; i += 1) {
      const line = lines[i];
      const trimmed = line.trim();
      if (isCommentLine(trimmed)) continue;

      if (isNavModule && SETTER_SHIM_RE.test(line)) {
        failures.push(
          `${file}:${i + 1} reintroduces a Dispatch<SetStateAction<...>> navigation setter shim. App-shell navigation transitions must be named reducer actions dispatched via useNavigationState's dispatch (audit F23), not setState-style cell writes.`,
        );
      }
      if (isNavModule && APPLY_ESCAPE_RE.test(line)) {
        failures.push(
          `${file}:${i + 1} reintroduces the generic APPLY navigation escape hatch. Add a named reducer action to useNavigationState instead of an anonymous (current) => state updater (audit F23).`,
        );
      }
      if (isSelectionConsumer && SELECTION_MIRROR_RE.test(line)) {
        failures.push(
          `${file}:${i + 1} mirrors the active selection into a ref. Read the live selection from the active-selection store via getActiveSelection() at event time instead of an effect-fed mirror (audit 6.4b).`,
        );
      }
    }
  }
  return failures;
}
