import { LIFECYCLE_ACTIVATION, readLifecycleSource } from "./guardrail-license-sources.mjs";
import { stripComments } from "./guardrail-source-text.mjs";

const COMMAND = "pub(super) async fn activate_license_with_ports";
const AFTER = "struct DesktopActivationPorts";
const REQUIRED = ["normalize_license_key(&key)", "activation_error(", "activation_error_from_raw("];

export function licenseActivationErrorFailures(read) {
  const path = LIFECYCLE_ACTIVATION;
  // Preserve string literals because the negative patterns inspect them.
  const source = stripComments(read(path), path);
  const start = source.indexOf(COMMAND);
  const end = start === -1 ? -1 : source.indexOf(AFTER, start);
  if (start === -1 || end === -1) {
    return [
      `${path} must define "${COMMAND}" ahead of "${AFTER}"; a rule that cannot find its bounds reports success over nothing at all`,
    ];
  }
  const body = source.slice(start, end);
  const failures = [];
  for (const required of REQUIRED) {
    if (!body.includes(required)) {
      failures.push(
        `${path} activate_license must call ${required} or raw LS strings reach the UI`,
      );
    }
  }
  if (
    /Err\(result\s*\.\s*error\s*\.\s*unwrap_or_else/.test(body) ||
    /Err\(\s*"Activation failed"\.to_string\(\)\s*\)/.test(body)
  ) {
    failures.push(
      `${path} activate_license must not return raw LS error strings or generic "Activation failed" text`,
    );
  }
  // The body check cannot establish that the referenced helper exists.
  if (!readLifecycleSource(read).includes("fn activation_error_from_raw")) {
    failures.push(`${path}: activation_error_from_raw is called but nothing defines it`);
  }
  return failures;
}
