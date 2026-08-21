import { telemetryReleaseFailures } from "./guardrail-telemetry-release-rules.mjs";

const TELEMETRY_WRAPPER = "apps/desktop/src/lib/telemetry.ts";
const TELEMETRY_TRANSPORT = "apps/desktop/src/lib/telemetry-transport.ts";
const TELEMETRY_COMMAND = "apps/desktop/src-tauri/src/commands/telemetry.rs";
const TELEMETRY_SCHEMA = "apps/desktop/src-tauri/src/commands/telemetry_schema.rs";

export function telemetryDisclosureFailures(read, exists) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  if (exists(TELEMETRY_WRAPPER)) {
    const wrapper = read(TELEMETRY_WRAPPER);
    const hostMatch = wrapper.match(/SENTRY_INGEST_HOST = "([^"]+)"/);
    check(hostMatch !== null, `${TELEMETRY_WRAPPER} must export a SENTRY_INGEST_HOST constant.`);
    check(
      !wrapper.includes("fetch(") &&
        wrapper.includes("tauriTelemetryTransport") &&
        wrapper.includes("diagnosticSender({ args: report })"),
      `${TELEMETRY_WRAPPER} must route usage telemetry and typed diagnostics through Rust, never renderer fetch.`,
    );

    const transport = exists(TELEMETRY_TRANSPORT) ? read(TELEMETRY_TRANSPORT) : "";
    const command = exists(TELEMETRY_COMMAND) ? read(TELEMETRY_COMMAND) : "";
    const schema = exists(TELEMETRY_SCHEMA) ? read(TELEMETRY_SCHEMA) : "";
    check(
      transport.includes("sendTelemetryRequest") &&
        transport.includes("const response = await sendTelemetryRequest({ args })") &&
        !transport.includes("fetch("),
      `${TELEMETRY_TRANSPORT} must use the typed Rust telemetry command and must not call renderer fetch.`,
    );
    check(
      hostMatch !== null &&
        command.includes(`const SENTRY_INGEST_HOST: &str = "${hostMatch?.[1]}"`) &&
        command.includes('const USAGE_TELEMETRY_HOST: &str = "telemetry.sitecmd.com"'),
      `${TELEMETRY_COMMAND} must keep exact telemetry host/path validation in sync with the public disclosure.`,
    );
    check(
      command.includes("validate_telemetry_target") &&
        command.includes("TelemetryConsentState") &&
        command.includes("require_consent") &&
        command.includes("configured_sentry_endpoint") &&
        command.includes("TelemetryRequestArgs") &&
        command.includes("credentialed_service_client()") &&
        command.includes("TELEMETRY_REQUEST_MAX_BYTES") &&
        command.includes("TELEMETRY_RESPONSE_MAX_BYTES") &&
        schema.includes("deny_unknown_fields") &&
        schema.includes("validate_ingest") &&
        schema.includes("sanitized_diagnostic"),
      `${TELEMETRY_COMMAND} must own persisted consent, fixed endpoints, closed request variants, strict body schemas, diagnostic reconstruction, no-redirect external DNS enforcement, and bounded request/response bodies.`,
    );
  }

  failures.push(...telemetryReleaseFailures(read, exists));
  return failures;
}
