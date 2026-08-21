export function desktopSharedTypeFailures(read) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const typesSource = read("apps/desktop/src/lib/types.ts");
  const severitySource = read("apps/desktop/src/lib/severity.ts");
  const confidenceSource = read("apps/desktop/src/lib/issue-confidence.ts");

  check(
    severitySource.includes("export const SEVERITIES") &&
      typesSource.includes('export type { Severity } from "./severity"') &&
      !typesSource.includes("export type Severity ="),
    "Desktop Severity type must be re-exported from lib/severity.ts, not duplicated in lib/types.ts.",
  );
  check(
    confidenceSource.includes(
      'const ISSUE_CONFIDENCES = ["confirmed", "high", "needs_review"] as const',
    ) &&
      confidenceSource.includes(
        "export type IssueConfidence = (typeof ISSUE_CONFIDENCES)[number]",
      ) &&
      typesSource.includes('export type { IssueConfidence } from "./issue-confidence"') &&
      !typesSource.includes("export type IssueConfidence ="),
    "Desktop IssueConfidence type must live with lib/issue-confidence.ts behavior and be re-exported from lib/types.ts.",
  );

  return failures;
}
