export function walkProdLines(source, onLine) {
  const lines = source.split("\n");
  let inTestMod = 0;
  let pendingTestMod = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (/^\s*#\[cfg\(test\)\]/.test(line)) {
      pendingTestMod = true;
      continue;
    }
    if (pendingTestMod) {
      if (/\bmod\b.*\{/.test(line)) {
        inTestMod = 1;
        pendingTestMod = false;
        continue;
      }
      if (/[a-zA-Z]/.test(line.trim())) pendingTestMod = false;
    }
    if (inTestMod > 0) {
      for (const ch of line) {
        if (ch === "{") inTestMod++;
        else if (ch === "}") inTestMod--;
        if (inTestMod === 0) break;
      }
      continue;
    }
    onLine(line, i, lines);
  }
}

function countBareUnwrapsExcludingTests(source) {
  let count = 0;
  walkProdLines(source, (line) => {
    if (line.includes(".unwrap()") && !line.includes("// allow-unwrap")) count++;
  });
  return count;
}

const RUST_PROD_DIRS = [
  "apps/desktop/src-tauri/src/background",
  "apps/desktop/src-tauri/src/commands",
  "apps/desktop/src-tauri/src/core",
  "apps/desktop/src-tauri/src/integrations",
  "apps/desktop/src-tauri/src/db",
  "apps/desktop/src-tauri/src/licensing",
  "apps/desktop/src-tauri/src/scoring",
  "apps/desktop/src-tauri/src/webview",
];
const RUST_PROD_SINGLE_FILES = ["apps/desktop/src-tauri/src/webhooks.rs"];

// Budgets may only decrease; unlisted files default to zero.
const UNWRAP_BUDGETS = new Map([
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/security/auth_session.rs", 179],
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/security/resilience_ai.rs", 131],
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/quality.rs", 111],
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/security/request_surface.rs", 110],
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/database.rs", 88],
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/operations.rs", 51],
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/security/commerce_redirects.rs", 49],
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/security/webhooks.rs", 16],
  ["apps/desktop/src-tauri/src/core/code_scan/patterns/packages.rs", 8],
  ["apps/desktop/src-tauri/src/commands/data/diagnostics.rs", 7],
  ["apps/desktop/src-tauri/src/commands/mod.rs", 1],
  ["apps/desktop/src-tauri/src/commands/scan/web_scan.rs", 1],
  ["apps/desktop/src-tauri/src/webview/analyzer.rs", 1],
]);

export function collectProdRustFiles(exists, listFiles) {
  return [
    ...RUST_PROD_DIRS.flatMap((dir) =>
      listFiles(dir, (file) => {
        if (!file.endsWith(".rs")) return false;
        if (/[/\\]tests[/\\]/.test(file)) return false;
        const name = file.split(/[/\\]/).pop();
        return !/(_tests?\.rs|^tests\.rs)$/.test(name);
      }),
    ),
    ...RUST_PROD_SINGLE_FILES.filter((file) => exists(file)),
  ];
}

export function rustUnwrapBudgetFailures(read, exists, listFiles) {
  const files = collectProdRustFiles(exists, listFiles);
  const failures = [];
  for (const file of files) {
    const count = countBareUnwrapsExcludingTests(read(file));
    const budget = UNWRAP_BUDGETS.get(file) ?? 0;
    if (count > budget) {
      failures.push(
        `${file}: ${count} bare \`.unwrap()\` (budget ${budget}); use \`.expect("static reason")\` or reduce the budget in tools/scripts/lib/guardrail-rust-rules.mjs`,
      );
    }
  }
  return failures;
}
