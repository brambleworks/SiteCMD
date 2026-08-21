export function fixGuideCspGuidanceFailures(read, listFiles) {
  const failures = [];

  const fixGuideCategoryFiles = listFiles("apps/desktop/src/lib/fix-guides", (file) =>
    file.endsWith(".ts"),
  );
  const cspFixGuidanceSources = [
    ...fixGuideCategoryFiles.map((file) => [file, read(file)]),
    ["apps/desktop/src-tauri/src/ai.rs", read("apps/desktop/src-tauri/src/ai.rs")],
  ];
  const unsafeScriptCspGuidance = cspFixGuidanceSources
    .filter(
      ([, source]) =>
        /script-src[^;\n]*'unsafe-inline'/.test(source) ||
        /script-src[^;\n]*'unsafe-eval'/.test(source) ||
        /onclick\s*=/.test(source),
    )
    .map(([file]) => file);
  if (unsafeScriptCspGuidance.length > 0) {
    failures.push(
      `SiteCMD CSP and HTML fix guidance must not recommend unsafe script CSP sources or inline event handlers: ${unsafeScriptCspGuidance.join(", ")}`,
    );
  }

  return failures;
}
