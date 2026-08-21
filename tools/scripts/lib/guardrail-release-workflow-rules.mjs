import { releasePipelineSafetyFailures } from "./guardrail-release-pipeline-rules.mjs";

// Compatibility entry point kept separate because the release pipeline has
// enough security invariants to warrant its own rule family.
export function releaseWorkflowSafetyFailures(read) {
  return releasePipelineSafetyFailures(read);
}
