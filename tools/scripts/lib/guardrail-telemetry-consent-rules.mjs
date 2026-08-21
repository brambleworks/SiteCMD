const TELEMETRY_WRAPPER = "apps/desktop/src/lib/telemetry.ts";

export function telemetryConsentFailures(read, exists) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const wrapper = exists(TELEMETRY_WRAPPER) ? read(TELEMETRY_WRAPPER) : "";
  const defaultConsent =
    wrapper.split("const DEFAULT_CONSENT: TelemetryConsentState = {").at(1)?.split("};").at(0) ??
    "";
  check(
    defaultConsent.includes("usageAnalytics: false") &&
      defaultConsent.includes("crashReports: false"),
    "Desktop telemetry must keep usage analytics and crash reports off until the user explicitly opts in.",
  );
  check(
    wrapper.includes("consentRevision += 1") &&
      wrapper.includes("if (consentRevision !== revisionAtStart) return") &&
      wrapper.includes("await persistConsent()"),
    "Desktop telemetry hydration must not overwrite a newer consent choice, and consent writes must await durable persistence.",
  );

  const optOutDiagnosticsCopy = ["apps/desktop/src/components/privacy/TelemetryConsentPrompt.tsx"]
    .filter((file) => exists(file))
    .filter((file) => /\bon by default\b|\bopt-out\b/i.test(read(file)));
  check(
    optOutDiagnosticsCopy.length === 0,
    `Telemetry copy must describe crash reports as opt-in, not on by default: ${optOutDiagnosticsCopy.join(", ")}`,
  );

  return failures;
}
