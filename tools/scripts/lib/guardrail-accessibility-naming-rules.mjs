const SCANNED_GLOBS = [
  "apps/desktop/src-tauri/src/checks/accessibility",
  "apps/desktop/src-tauri/src/core",
  "apps/desktop/src-tauri/src/commands",
  "apps/desktop/src-tauri/src/cli",
  "apps/desktop/src-tauri/src/ai.rs",
  "apps/desktop/src/lib",
  "apps/desktop/src/components",
];

const A11Y_RE = /\ba11y\b/i;
const ALLOWED_LITERAL_SUBSTRINGS = ["jsx-a11y", "plugin-jsx-a11y"];

export function accessibilityNamingFailures(read, exists, listFiles) {
  const failures = [];
  for (const root of SCANNED_GLOBS) {
    if (!exists(root)) continue;
    const files = root.endsWith(".rs")
      ? [root]
      : listFiles(root, (file) => /\.(rs|ts|tsx)$/.test(file));
    for (const file of files) {
      const lines = read(file).split("\n");
      for (let i = 0; i < lines.length; i += 1) {
        const line = lines[i];
        if (!A11Y_RE.test(line)) continue;
        if (ALLOWED_LITERAL_SUBSTRINGS.some((sub) => line.includes(sub))) continue;
        failures.push(
          `${file}:${i + 1} - uses "a11y" - spell out "Accessibility". Line: ${line.trim()}`,
        );
      }
    }
  }
  return failures;
}
