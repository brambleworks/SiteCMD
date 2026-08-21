// The separately published MCP package mirrors desktop severity vocabulary;
// a repository guardrail enforces parity.
const SEVERITY_ORDER = ["critical", "high", "medium", "low"] as const;

type KnownSeverity = (typeof SEVERITY_ORDER)[number];

function severityIndex(severity: string): number {
  return SEVERITY_ORDER.indexOf(severity as KnownSeverity);
}

export function severityRank(severity: string): number {
  const index = severityIndex(severity);
  return index === -1 ? SEVERITY_ORDER.length : index;
}

export function severityMatchesMinimum(issueSeverity: string, minimumSeverity: string): boolean {
  const issueIndex = severityIndex(issueSeverity);
  const minimumIndex = severityIndex(minimumSeverity);
  if (issueIndex === -1 || minimumIndex === -1) {
    return issueSeverity === minimumSeverity;
  }
  return issueIndex <= minimumIndex;
}

export function severitiesAtOrAbove(minimumSeverity: string): string[] {
  const minimumIndex = severityIndex(minimumSeverity);
  if (minimumIndex === -1) return [minimumSeverity];
  return [...SEVERITY_ORDER.slice(0, minimumIndex + 1)];
}
