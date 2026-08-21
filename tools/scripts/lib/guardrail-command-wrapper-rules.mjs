const ALLOWED_EXACT = new Set([
  "apps/desktop/src/lib/tauri-invoke.ts",
  "apps/desktop/src/lib/privileged-command-bridge.ts",
]);
const ALLOWED_PREFIX = "apps/desktop/src/lib/commands/";

// Match the transport invoke function, not methods or longer identifiers.
const INVOKE_CALL_RE = /(?<![.\w])invoke\s*[<(]/;
// Importing `invoke` from the transport module or straight from tauri core.
const INVOKE_IMPORT_RE =
  /import\s[^;]*\binvoke\b[^;]*from\s+["'](?:@\/lib\/tauri-invoke|@tauri-apps\/api\/core)["']/;

function isTestFile(file) {
  return /\.(test|spec)\.[cm]?[jt]sx?$/.test(file);
}

export function commandWrapperFailures(read, sourceFiles) {
  const failures = [];
  for (const file of sourceFiles) {
    if (!/\.(ts|tsx)$/.test(file)) continue;
    if (isTestFile(file)) continue;
    if (ALLOWED_EXACT.has(file) || file.startsWith(ALLOWED_PREFIX)) continue;

    const source = read(file);
    if (INVOKE_IMPORT_RE.test(source)) {
      failures.push(
        `${file} imports \`invoke\` from the transport layer. Call a typed wrapper from @/lib/commands instead (add one there if the command has no wrapper yet).`,
      );
      continue;
    }

    const lines = source.split("\n");
    for (let i = 0; i < lines.length; i += 1) {
      const trimmed = lines[i].trim();
      if (trimmed.startsWith("//") || trimmed.startsWith("*")) continue;
      if (INVOKE_CALL_RE.test(lines[i])) {
        failures.push(
          `${file}:${i + 1} calls invoke(...) directly. Route it through a typed wrapper in @/lib/commands so the command name, arg keys, and return type are checked against the generated IPC bindings.`,
        );
      }
    }
  }
  return failures;
}
