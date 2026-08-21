export function telemetryReleaseFailures(read, exists) {
  const releaseWorkflow = exists(".github/workflows/release.yml")
    ? read(".github/workflows/release.yml")
    : "";
  const hasUsageEndpoint = releaseWorkflow.includes(
    'VITE_SITECMD_TELEMETRY_ENDPOINT: "https://telemetry.sitecmd.com/v1/events"',
  );
  const hasSentryDsn = releaseWorkflow.includes(
    "VITE_SITECMD_SENTRY_DSN: ${{ secrets.SITECMD_SENTRY_DSN }}",
  );
  return hasUsageEndpoint && hasSentryDsn
    ? []
    : [
        "release.yml must bake VITE_SITECMD_TELEMETRY_ENDPOINT (https://telemetry.sitecmd.com/v1/events) and VITE_SITECMD_SENTRY_DSN into the Tauri build env; without them shipped builds have telemetry and crash reporting inert.",
      ];
}
