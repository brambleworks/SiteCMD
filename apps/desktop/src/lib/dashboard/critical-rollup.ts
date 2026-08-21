import type { CriticalRollup } from "./types";

export function buildCriticalRollup(input: {
  criticalWebIssues?: number;
  criticalCodeIssues?: number;
  securityPatchCount?: number;
}): CriticalRollup {
  const web = input.criticalWebIssues ?? 0;
  const code = input.criticalCodeIssues ?? 0;
  const securityPatches = input.securityPatchCount ?? 0;
  return { total: web + code + securityPatches, web, code, securityPatches };
}
