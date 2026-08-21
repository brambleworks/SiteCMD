// Match direct calls with or without a generic type argument.
const SAFE_LISTEN_RE = /\bsafeListen\s*(<[^>]*>)?\s*\(/;

const SAFE_LISTEN_ALLOWLIST = new Set([
  "apps/desktop/src/lib/tauri-events.ts",
  "apps/desktop/src/hooks/useTauriEvent.ts",
  "apps/desktop/src/lib/query/event-invalidation.ts",
  "apps/desktop/src/lib/app-update.ts",
  "apps/desktop/src/hooks/useScan.ts",
]);

function isTestFile(file) {
  return /\.(test|spec)\.[cm]?[jt]sx?$/.test(file);
}

function isCommentLine(trimmed) {
  return trimmed.startsWith("//") || trimmed.startsWith("*");
}

export function eventFabricFailures(read, sourceFiles) {
  const failures = [];
  for (const file of sourceFiles) {
    if (!/\.(ts|tsx)$/.test(file)) continue;
    if (isTestFile(file)) continue;
    if (SAFE_LISTEN_ALLOWLIST.has(file)) continue;

    const lines = read(file).split("\n");
    for (let i = 0; i < lines.length; i += 1) {
      const trimmed = lines[i].trim();
      if (isCommentLine(trimmed)) continue;
      if (SAFE_LISTEN_RE.test(lines[i])) {
        failures.push(
          `${file}:${i + 1} calls safeListen directly. Subscribe through useTauriEvent (hooks/useTauriEvent) with a name registered in lib/app-events, or register a query-key invalidation in lib/query/event-invalidation. Raw safeListen effect scaffolds are the leak-prone pattern audit F14 removed.`,
        );
      }
    }
  }
  return failures;
}
