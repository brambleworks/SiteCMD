const POLICY_DEFINITION = "apps/desktop/src-tauri/src/core/severity_policy.rs";

const CHOKEPOINTS = [
  {
    fn: "normalize_check_results",
    file: "apps/desktop/src-tauri/src/core/scanner/finalize.rs",
    // Every web assembly path must route through the finalizer.
    assemblySites: [
      "apps/desktop/src-tauri/src/core/scanner.rs",
      "apps/desktop/src-tauri/src/core/scanner/verify.rs",
      "apps/desktop/src-tauri/src/core/scanner/webview_results.rs",
      "apps/desktop/src-tauri/src/commands/scan/multi_scan.rs",
    ],
  },
  {
    fn: "normalize_code_issues",
    file: "apps/desktop/src-tauri/src/core/code_scan/mod.rs",
    assemblySites: [],
  },
];

export function severityPolicyChokepointFailures(read, listFiles) {
  const files = listFiles("apps/desktop/src-tauri/src", (file) => {
    if (!file.endsWith(".rs")) return false;
    if (/[/\\]tests[/\\]/.test(file)) return false;
    const name = file.split(/[/\\]/).pop();
    return !/(_tests?\.rs|^tests\.rs)$/.test(name);
  });
  const failures = [];
  for (const { fn: fnName, file: chokepoint, assemblySites } of CHOKEPOINTS) {
    const rogueCallers = files.filter(
      (file) =>
        file !== chokepoint && file !== POLICY_DEFINITION && read(file).includes(`${fnName}(`),
    );
    if (rogueCallers.length > 0) {
      failures.push(
        `severity_policy::${fnName} may only be called from ${chokepoint}; route new assembly paths through the chokepoint instead: ${rogueCallers.join(", ")}`,
      );
    }
    if (!read(chokepoint).includes(`severity_policy::${fnName}(`)) {
      failures.push(
        `${chokepoint} must call severity_policy::${fnName} so every assembled scan result passes through severity policy.`,
      );
    }
    for (const site of assemblySites) {
      if (!read(site).includes("finalize_check_results(")) {
        failures.push(
          `${site} assembles web check results and must route them through finalize_check_results (core/scanner/finalize.rs).`,
        );
      }
    }
  }
  return failures;
}
