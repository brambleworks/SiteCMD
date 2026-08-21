export function desktopIssueStatusFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const issuesSource = read("apps/desktop/src/lib/issues.ts");
  const issuesTests = read("apps/desktop/src/lib/issues.test.ts");

  const inlineWebIssueStatusFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/") || !/\.(ts|tsx)$/.test(file)) return false;
    if (file === "apps/desktop/src/lib/issues.ts") return false;
    if (file.includes(".test.") || file.includes(".capture.")) return false;
    const source = read(file);
    return (
      /\w+\.status === ["']fail["']\s*\|\|\s*\w+\.status === ["']warn["']/.test(source) ||
      /\w+\.status !== ["']pass["']/.test(source)
    );
  });

  check(
    issuesSource.includes("isActionableCheckStatus") &&
      issuesSource.includes("isActionableCheckResult") &&
      issuesSource.includes("filterActionableCheckResults") &&
      issuesSource.includes("countActionableCheckResults") &&
      issuesTests.includes("treats fail and warn as actionable web check statuses") &&
      inlineWebIssueStatusFiles.length === 0,
    `Desktop web-check actionable status logic must use lib/issues.ts helpers instead of local fail/warn or not-pass predicates: ${inlineWebIssueStatusFiles.join(", ")}`,
  );

  const inlineCheckStatusLabelFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/") || !/\.(ts|tsx)$/.test(file)) return false;
    if (file === "apps/desktop/src/lib/issues.ts") return false;
    if (file.includes(".test.") || file.includes(".capture.")) return false;
    return /return ["']Pass["'];[\s\S]*return ["']Fail["'];[\s\S]*return ["']Warn["'];[\s\S]*return ["']Skipped["'];/.test(
      read(file),
    );
  });
  check(
    issuesSource.includes("formatCheckStatus") &&
      issuesTests.includes("formats web check statuses from one shared label helper") &&
      inlineCheckStatusLabelFiles.length === 0,
    `Desktop web-check status labels must use lib/issues.ts formatCheckStatus instead of local Pass/Fail/Warn/Skipped switches: ${inlineCheckStatusLabelFiles.join(", ")}`,
  );

  return failures;
}
