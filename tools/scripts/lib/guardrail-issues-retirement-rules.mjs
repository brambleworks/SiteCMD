export function desktopIssuesRetirementFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const retiredVerificationCopyFiles = sourceFiles.filter(
    (file) =>
      file.startsWith("apps/desktop/src/") &&
      /\.(ts|tsx)$/.test(file) &&
      !/\.(test|spec)\.(ts|tsx)$/.test(file) &&
      /waiting for verification|pending verification/i.test(read(file)),
  );
  check(
    retiredVerificationCopyFiles.length === 0,
    `Desktop UI must not resurrect the retired waiting-verification queue copy: ${retiredVerificationCopyFiles.join(", ")}`,
  );

  check(
    !sourceFiles.includes("apps/desktop/src/pages/issues/IssuesPageContextRail.tsx") &&
      !sourceFiles.includes("apps/desktop/src/pages/issues-attention.ts"),
    "Issues page must not resurrect a follow-up banner rail above the queue.",
  );

  return failures;
}
