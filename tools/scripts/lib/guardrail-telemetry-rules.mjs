import path from "node:path";
import { objectValueExpression, stripJsComments } from "./guardrail-text-utils.mjs";

const TELEMETRY_WRAPPER = "apps/desktop/src/lib/telemetry.ts";
const TELEMETRY_TRANSPORT = "apps/desktop/src/lib/telemetry-transport.ts";
const DESKTOP_PACKAGE = "apps/desktop/package.json";
const VITE_CONFIG = "apps/desktop/vite.config.ts";
const SENTRY_IMPORT_RE = /from\s+["']@sentry\/|import\(["']@sentry\//;
const TELEMETRY_ENDPOINT_RE = /VITE_SITECMD_TELEMETRY_ENDPOINT|telemetry\.sitecmd\.com/;
const isTelemetryBoundaryFile = (file) =>
  file === TELEMETRY_WRAPPER ||
  file === TELEMETRY_TRANSPORT ||
  /\.test\./.test(file) ||
  /\.d\.ts$/.test(file);
function blockStartingAt(source, marker) {
  const start = source.indexOf(marker);
  const open = start < 0 ? -1 : source.indexOf("{", start + marker.length);
  if (open === -1) return null;
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] !== "}") continue;
    depth -= 1;
    if (depth === 0) return source.slice(open, index + 1);
  }
  return null;
}

export function telemetrySafetyFailures(read, exists, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const desktopFiles = listFiles("apps/desktop/src", (file) => /\.(ts|tsx)$/.test(file));
  const sentryBypassFiles = desktopFiles.filter((file) => SENTRY_IMPORT_RE.test(read(file)));
  check(
    sentryBypassFiles.length === 0 &&
      (!exists(DESKTOP_PACKAGE) || !read(DESKTOP_PACKAGE).includes('"@sentry/browser"')),
    `The renderer must not embed a generic Sentry client; diagnostics are typed and constructed by Rust: ${sentryBypassFiles.join(", ")}`,
  );

  const endpointBypassFiles = desktopFiles.filter((file) => {
    if (isTelemetryBoundaryFile(file)) return false;
    return TELEMETRY_ENDPOINT_RE.test(read(file));
  });
  check(
    endpointBypassFiles.length === 0,
    `Desktop telemetry endpoint usage must stay inside ${TELEMETRY_WRAPPER}: ${endpointBypassFiles.join(", ")}`,
  );

  if (exists(TELEMETRY_WRAPPER)) {
    const wrapper = read(TELEMETRY_WRAPPER);
    check(
      wrapper.includes("getTelemetryConsent") &&
        wrapper.includes("setBackendTelemetryConsent") &&
        wrapper.includes("usageAnalytics: false") &&
        wrapper.includes("crashReports: false") &&
        wrapper.includes("diagnosticSender({ args: report })") &&
        !wrapper.includes("fetch("),
      "Telemetry must start fail-closed, hydrate and mutate consent through the Rust authority, and submit diagnostics only as typed native reports.",
    );
    check(
      wrapper.includes("MAX_QUEUED_EVENT_AGE_MS") &&
        wrapper.includes("queuedEventIsWithinAcceptanceWindow") &&
        wrapper.includes(
          ".filter((event) => queuedEventIsWithinAcceptanceWindow(event.occurredAt))",
        ) &&
        read("apps/desktop/src/lib/telemetry.test.ts").includes(
          "drops queued usage events after the server acceptance window",
        ),
      "Desktop telemetry must prune queued events that the hosted acceptance window will reject.",
    );
    // Both payloads must report the package version and build channel.
    const viteConfig = stripJsComments(exists(VITE_CONFIG) ? read(VITE_CONFIG) : "");
    const defineValue = objectValueExpression(viteConfig, "import.meta.env.VITE_APP_VERSION");
    check(
      /^JSON\.stringify\(\s*[A-Za-z_$][\w$]*\s*\)$/.test(defineValue) &&
        /\breadFileSync\s*\(/.test(viteConfig) &&
        viteConfig.includes("package.json") &&
        wrapper.includes("appVersion: APP_VERSION") &&
        wrapper.includes("buildChannel: BUILD_CHANNEL"),
      `Telemetry and crash reports must name the shipped build: ${VITE_CONFIG} must define import.meta.env.VITE_APP_VERSION by reading package.json, not from a literal, and both typed payloads must carry appVersion + buildChannel.`,
    );
  }

  const appContentPath = "apps/desktop/src/app/AppContent.tsx";
  const appShellHelpersPath = "apps/desktop/src/app/app-shell-helpers.ts";
  if (exists(appContentPath)) {
    const appContent = stripJsComments(read(appContentPath));
    const shellHelpers = exists(appShellHelpersPath)
      ? stripJsComments(read(appShellHelpersPath))
      : "";
    const bootstrapBranch = blockStartingAt(appContent, "if (bootstrapState)");
    check(
      bootstrapBranch !== null && !bootstrapBranch.includes("<TelemetryConsentPrompt"),
      "Telemetry consent prompt must not render inside the StartupShell bootstrap branch.",
    );
    check(
      appContent.includes("useHasCompletedFirstScan") &&
        appContent.includes("shouldShowTelemetryConsentPrompt({") &&
        appContent.includes("<TelemetryConsentPrompt />") &&
        shellHelpers.includes("if (!hasCompletedFirstScan || projectCount === 0) return false;") &&
        shellHelpers.includes("return !showScanSummary && !showFirstRunWalkthrough;"),
      "AppContent must gate <TelemetryConsentPrompt /> through shouldShowTelemetryConsentPrompt(), which requires useHasCompletedFirstScan() and hides the prompt behind the scan summary and the first-run walkthrough.",
    );
  }

  return failures.map((failure) => path.normalize(failure));
}
