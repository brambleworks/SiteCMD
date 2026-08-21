const RUST_FILE_LINE_LIMIT = 800;

const rustLineBudgetOverrides = new Map([
  // Overrides require a reason that splitting would be worse.
]);

function isProdRustFile(rustFile) {
  if (/[/\\]tests[/\\]/.test(rustFile)) return false;
  return !/(_tests?\.rs|^tests\.rs)$/.test(rustFile.split(/[/\\]/).pop());
}

export function rustLineBudgetFailures(read, listFiles) {
  const tauriRustFiles = [
    ...listFiles("apps/desktop/src-tauri/src", (file) => file.endsWith(".rs")),
    ...listFiles("apps/desktop/src-tauri/crates", (file) => file.endsWith(".rs")),
  ];
  const failures = [];
  for (const rustFile of tauriRustFiles) {
    if (!isProdRustFile(rustFile)) continue;
    const lineCount = read(rustFile).split("\n").length;
    const maxLines = rustLineBudgetOverrides.get(rustFile) ?? RUST_FILE_LINE_LIMIT;
    if (lineCount > maxLines) {
      failures.push(`${rustFile} has ${lineCount} lines (budget ${maxLines})`);
    }
  }
  if (failures.length === 0) return [];
  return [
    `Rust source files exceeded maintainability line budgets. Split into submodules or add a per-file override (with justification) in tools/scripts/lib/guardrail-rust-loc-rules.mjs: ${failures.join(", ")}`,
  ];
}
