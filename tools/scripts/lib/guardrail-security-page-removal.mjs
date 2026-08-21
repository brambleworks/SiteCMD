export function desktopSecurityPageRemovalFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const resurrectedSecurityFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/")) return false;
    return (
      /(^|\/)SecurityPage\.(tsx?|test\.(tsx?|ts))$/.test(file) ||
      /(^|\/)SecurityPageSections\.tsx?$/.test(file) ||
      /(^|\/)SecurityContextPanel\.tsx?$/.test(file) ||
      /(^|\/)SecurityNpmAuditCard\.tsx?$/.test(file) ||
      /(^|\/)useSecurityPageData\.ts$/.test(file) ||
      /(^|\/)useSecurityReportExport\.ts$/.test(file) ||
      /(^|\/)security-page-(?:model|cache)\.ts$/.test(file) ||
      /(^|\/)security-scan-loader\.ts$/.test(file) ||
      /(^|\/)security-report-export\.ts$/.test(file)
    );
  });
  check(
    resurrectedSecurityFiles.length === 0,
    `Security page was deleted in favor of Issues?category=security. Do not reintroduce these files: ${resurrectedSecurityFiles.join(", ")}`,
  );

  const navSidebarSource = read("apps/desktop/src/components/layout/NavSidebar.tsx");
  check(
    !/page: "security"/.test(navSidebarSource) && !/\| "security"\b/.test(navSidebarSource),
    "NavSidebar must not reintroduce a top-level 'security' entry. Security is a category filter inside Issues.",
  );

  return failures;
}
