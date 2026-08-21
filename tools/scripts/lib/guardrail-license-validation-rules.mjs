import { LIFECYCLE_VALIDATION, readLifecycleSource } from "./guardrail-license-sources.mjs";

function licenseValidationDowngradeGateFailures(read) {
  const path = LIFECYCLE_VALIDATION;
  const source = readLifecycleSource(read);
  const failures = [];
  if (
    !source.includes("offline_validation_or_downgrade") ||
    !source.includes("classify_offline_validation") ||
    !source.includes("OfflineValidationState::Expired")
  ) {
    failures.push(
      `${path} must route the LemonSqueezy network-failure branch through offline_validation_or_downgrade() so the cached tier never silently downgrades before the StaleFinalWarning banner fires`,
    );
  }
  return failures;
}

// Aggregator: one import for the runner, every license-lifecycle gate.
import { licenseActivationErrorFailures } from "./guardrail-license-activation-rules.mjs";
import { licenseValidationBranchFailures } from "./guardrail-license-validation-branch-rules.mjs";
export function licenseLifecycleSafetyFailures(read) {
  return [
    ...licenseValidationDowngradeGateFailures(read),
    ...licenseValidationBranchFailures(read),
    ...licenseActivationErrorFailures(read),
  ];
}
