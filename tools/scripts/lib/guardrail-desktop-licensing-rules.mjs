import { licenseFeatureSourceFailures } from "./guardrail-desktop-feature-rules.mjs";
import { LIFECYCLE_TESTS, readLifecycleSource } from "./guardrail-license-sources.mjs";

// Test-name checks inspect dedicated test modules.
const API = "apps/desktop/src-tauri/src/licensing/api.rs";
const API_TESTS = "apps/desktop/src-tauri/src/licensing/api_tests.rs";
const COMMANDS_TESTS = "apps/desktop/src-tauri/src/licensing/commands/mod_tests.rs";

export function desktopLicensingSafetyFailures(read) {
  return [
    ...licenseFeatureSourceFailures(read),
    ...licenseActivationAuditFailures(read),
    ...licenseInstanceNameFailures(read),
    ...lemonCheckoutTrialStatusFailures(read),
  ];
}

function licenseActivationAuditFailures(read) {
  // Production and test assertions intentionally read separate sources.
  const productionSource = readLifecycleSource(read);
  const failures = [];

  if (
    !productionSource.includes("fn license_key_fingerprint") ||
    !productionSource.includes('"key_fingerprint"') ||
    productionSource.includes('"key_prefix"') ||
    productionSource.includes("chars().take(8)")
  ) {
    failures.push("apps/desktop/src-tauri/src/licensing/commands/license_lifecycle*.rs");
  }

  if (
    !read(LIFECYCLE_TESTS).includes(
      "license_activation_audit_detail_uses_fingerprint_not_key_prefix",
    )
  ) {
    failures.push(`${LIFECYCLE_TESTS} must keep the fingerprint audit test`);
  }

  return failures;
}

function licenseInstanceNameFailures(read) {
  const source = read(API);
  const failures = [];

  if (
    !source.includes("fn machine_instance_name_from_parts") ||
    !source.includes('format!("sitecmd-{}"') ||
    source.includes('format!("{}-{:08x}"') ||
    source.includes("hostname + a hash")
  ) {
    failures.push(API);
  }

  if (!read(API_TESTS).includes("machine_instance_name_does_not_leak_host_or_username")) {
    failures.push(`${API_TESTS} must keep the instance-name leak test`);
  }

  return failures;
}

function lemonCheckoutTrialStatusFailures(read) {
  const accessSource = read("apps/desktop/src-tauri/src/licensing/access.rs");
  const commandTests = read(COMMANDS_TESTS);
  const apiTests = read(API_TESTS);
  const failures = [];

  if (
    !accessSource.includes('matches!(status, "active" | "on_trial")') ||
    !accessSource.includes("effective_tier_keeps_recent_lemon_checkout_trial_cache") ||
    !commandTests.includes("validation_result_preserves_lemon_checkout_trial_status") ||
    !commandTests.includes("info_from_state_treats_lemon_checkout_trial_as_active") ||
    !apiTests.includes("parse_activate_response_accepts_lemon_checkout_trial_status") ||
    !apiTests.includes("parse_validate_response_accepts_lemon_checkout_trial_status")
  ) {
    failures.push("Lemon Squeezy on_trial license status must remain entitled and tested");
  }

  return failures;
}
