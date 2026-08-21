const QUERY_LAYER_PREFIX = "apps/desktop/src/lib/query/";
const TEST_UTILS_PREFIX = "apps/desktop/src/test-utils/";
const PROJECT_SUMMARY_SIGNALS_FILE = "apps/desktop/src/lib/project-summary-signals.ts";
// Type annotations are allowed; only a private SnapshotCacheEntry Map is banned.
const SNAPSHOT_MAP_RE = /new\s+Map<[^;]*SnapshotCacheEntry/;

const NEW_QUERY_CLIENT_RE = /new\s+QueryClient\s*\(/;
// Match an inline query-key array, not a registered key or variable.
const INLINE_QUERY_KEY_RE = /queryKey:\s*\[/;

// Event writes must emit through the adapter that invalidates cached activity.
const EVENT_WRITES_FILE = "apps/desktop/src/lib/event-writes.ts";
const RAW_EVENT_RECORDER_RE = /\brecord(Search|Update)Event\b/;
const COMMANDS_IMPORT_RE = /import\s*\{([^}]*)\}\s*from\s*["']@\/lib\/commands["']/g;

function isTestFile(file) {
  return /\.(test|spec)\.[cm]?[jt]sx?$/.test(file);
}

function isCommentLine(trimmed) {
  return trimmed.startsWith("//") || trimmed.startsWith("*");
}

export function queryLayerFailures(read, sourceFiles) {
  const failures = [];
  for (const file of sourceFiles) {
    if (!/\.(ts|tsx)$/.test(file)) continue;
    if (isTestFile(file)) continue;
    if (file.startsWith(QUERY_LAYER_PREFIX) || file.startsWith(TEST_UTILS_PREFIX)) continue;

    const lines = read(file).split("\n");
    for (let i = 0; i < lines.length; i += 1) {
      const trimmed = lines[i].trim();
      if (isCommentLine(trimmed)) continue;
      if (NEW_QUERY_CLIENT_RE.test(lines[i])) {
        failures.push(
          `${file}:${i + 1} constructs a QueryClient. There must be one client - create it in lib/query/query-client.ts and consume it through AppQueryProvider / useQueryClient.`,
        );
      }
      if (INLINE_QUERY_KEY_RE.test(lines[i])) {
        failures.push(
          `${file}:${i + 1} uses an inline query-key array. Register the key in lib/query/query-keys.ts and reference \`queryKeys.*\` so the event-invalidation registry can find it.`,
        );
      }
    }

    if (file !== EVENT_WRITES_FILE) {
      // Imports may span lines.
      for (const match of read(file).matchAll(COMMANDS_IMPORT_RE)) {
        if (!RAW_EVENT_RECORDER_RE.test(match[1])) continue;
        failures.push(
          `${file} imports a raw event recorder from @/lib/commands. Import it from lib/event-writes instead, so the write is paired with the \`events-recorded\` emit that refreshes the cached Activity ranges.`,
        );
      }
    }
  }

  const signalsSource = read(PROJECT_SUMMARY_SIGNALS_FILE);
  if (!signalsSource.includes("queryKeys.projectSummary")) {
    failures.push(
      `${PROJECT_SUMMARY_SIGNALS_FILE} must cache the dashboard summary payloads through queryKeys.projectSummary (the shared QueryClient), not a bespoke store - that was the F12 collapse.`,
    );
  }
  if (SNAPSHOT_MAP_RE.test(signalsSource)) {
    failures.push(
      `${PROJECT_SUMMARY_SIGNALS_FILE} reintroduced a module-level SnapshotCacheEntry Map. Dashboard summary caches live in the shared QueryClient (queryKeys.projectSummary) - use getQueryData/setQueryData, not a private Map.`,
    );
  }

  return failures;
}
