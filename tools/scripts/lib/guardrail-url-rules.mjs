export function desktopUrlIdentityFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const appTargetsSource = read("apps/desktop/src/lib/app-targets.ts");
  const appTargetsTests = read("apps/desktop/src/lib/app-targets.test.ts");
  const inlineUrlKeyFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/") || !/\.(ts|tsx)$/.test(file)) return false;
    if (file === "apps/desktop/src/lib/app-targets.ts") return false;
    if (file === "apps/desktop/src/lib/utils.ts") return false;
    if (file.includes(".test.")) return false;

    const source = read(file)
      .split("\n")
      .filter(
        (line) =>
          !line.includes("projectPath.replace(/\\/$/") &&
          !line.includes("parsed.pathname.replace(/\\/$/"),
      )
      .join("\n");
    return source.includes('replace(/\\/$/, "")') || source.includes("replace(/\\/$/, '')");
  });

  check(
    appTargetsSource.includes("normalizeAppUrlForKey") &&
      appTargetsSource.includes("normalizeAppUrlForOptionalKey") &&
      appTargetsTests.includes("normalizes http URLs consistently for cache and work-item keys") &&
      inlineUrlKeyFiles.length === 0,
    `Desktop URL identity/cache/work-item keys must use app-targets.ts normalizeAppUrlForKey instead of local trailing-slash regex copies: ${inlineUrlKeyFiles.join(", ")}`,
  );

  return failures;
}
