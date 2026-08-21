import type { DashboardSnapshotInputs, SiteVerdict } from "./types";

const MAX_REASONS = 3;

export function deriveSiteVerdict(input: DashboardSnapshotInputs): SiteVerdict {
  const blockedReasons: string[] = [];
  const attentionReasons: string[] = [];

  if (input.criticalWebIssues > 0) {
    blockedReasons.push(
      `${input.criticalWebIssues} critical web issue${input.criticalWebIssues === 1 ? "" : "s"}`,
    );
  }
  if (input.criticalCodeIssues > 0) {
    blockedReasons.push(
      `${input.criticalCodeIssues} critical code issue${input.criticalCodeIssues === 1 ? "" : "s"}`,
    );
  }
  if (input.deployFailed) {
    blockedReasons.push("Latest deploy failed");
  }
  if (input.sslDaysRemaining !== null && input.sslDaysRemaining < 14) {
    blockedReasons.push(`SSL ${input.sslDaysRemaining}d`);
  }

  if (input.securityPatchCount > 0) {
    attentionReasons.push(
      `${input.securityPatchCount} security patch${input.securityPatchCount === 1 ? "" : "es"} pending`,
    );
  }
  if (input.integrationFailureCount > 0) {
    attentionReasons.push(`${input.integrationFailureCount} integration failing`);
  }
  if (input.staleIntegrationCount > 0) {
    attentionReasons.push(
      `${input.staleIntegrationCount} stale source${input.staleIntegrationCount === 1 ? "" : "s"}`,
    );
  }
  if (input.searchRegressionNegative) {
    attentionReasons.push("Search traffic down");
  }
  if (
    input.sslDaysRemaining !== null &&
    input.sslDaysRemaining >= 14 &&
    input.sslDaysRemaining < 30
  ) {
    attentionReasons.push(`SSL ${input.sslDaysRemaining}d`);
  }

  if (blockedReasons.length > 0) {
    return {
      kind: "blocked",
      phrase: "Blocked",
      reasons: blockedReasons.slice(0, MAX_REASONS),
    };
  }
  if (attentionReasons.length > 0) {
    return {
      kind: "attention",
      phrase: "Attention needed",
      reasons: attentionReasons.slice(0, MAX_REASONS),
    };
  }
  return { kind: "healthy", phrase: "Healthy", reasons: [] };
}
