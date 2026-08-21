import { Fragment, StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";

// Keep react-scan opt-in because it can disrupt the webview's dev IPC fallback.
const shouldEnableReactScan =
  import.meta.env.DEV &&
  import.meta.env.VITE_ENABLE_REACT_SCAN === "1" &&
  typeof window !== "undefined" &&
  !("__TAURI_INTERNALS__" in window);

if (shouldEnableReactScan) {
  void import("react-scan").then(({ scan }) => {
    scan({ enabled: true, log: false });
  });
}

import { AppQueryProvider } from "./lib/query/AppQueryProvider";
import { ScanPrefsProvider } from "./hooks/useScanPrefs";
import { ThemeProvider } from "./hooks/useTheme";
import { ToastProvider } from "./hooks/useToast";
import { logger, installGlobalErrorHandlers } from "./lib/logger";
import { errorMessage } from "./lib/error-message";
import { recordErrorReport } from "./lib/observability";
import { finishPerformanceTimerAfterPaint, startPerformanceTimer } from "./lib/performance-metrics";
import { initializeTelemetryFromStoredConsent } from "./lib/telemetry";
import {
  installPrivilegedCommandBridge,
  isPrivilegedBridgeWindow,
} from "./lib/privileged-command-bridge";
import {
  markStartupStage,
  renderStartupFallback,
  startStartupWatchdog,
  supportsRequiredWebviewFeatures,
} from "./lib/startup-guard";

const privilegedBridgeWindow = isPrivilegedBridgeWindow();

installGlobalErrorHandlers();
if (!privilegedBridgeWindow) initializeTelemetryFromStoredConsent();
markStartupStage("booting");
const coldStartTimer = startPerformanceTimer("app.cold_start_ms");

const stopStartupWatchdog = startStartupWatchdog({
  timeoutMs: 8000,
  onTimeout: () => {
    recordErrorReport("startup.watchdog", "Frontend bootstrap watchdog timed out", {
      fatal: true,
    });
    logger.error("Frontend bootstrap watchdog timed out before mount completed");
    renderStartupFallback({
      title: "SiteCMD did not finish loading",
      description:
        "The app shell never finished booting. Reload the app, and if this keeps happening, reset the saved state to escape a bad startup loop.",
      details:
        "If you are using the attach flow in development, make sure the Vite dev server is already running at http://127.0.0.1:5173 before attaching Tauri.",
    });
  },
});

async function bootstrap() {
  if (privilegedBridgeWindow) {
    await installPrivilegedCommandBridge();
    stopStartupWatchdog();
    markStartupStage("mounted");
    return;
  }

  if (!supportsRequiredWebviewFeatures()) {
    stopStartupWatchdog();
    renderStartupFallback({
      title: "SiteCMD needs a newer system webview",
      description:
        "Update macOS or Microsoft Edge WebView2, or install WebKitGTK 2.40 or later on Linux, then reload SiteCMD.",
      showResetAction: false,
    });
    return;
  }

  const rootElement = document.getElementById("root");
  if (!rootElement) {
    recordErrorReport("startup.bootstrap", "#root element is missing", {
      fatal: true,
    });
    logger.error("Frontend bootstrap failed: #root element is missing");
    return;
  }

  try {
    const { default: App } = await import("./App");
    const RootMode =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window ? Fragment : StrictMode;

    createRoot(rootElement).render(
      <RootMode>
        <AppQueryProvider>
          <ThemeProvider>
            <ScanPrefsProvider>
              <ToastProvider>
                <App />
              </ToastProvider>
            </ScanPrefsProvider>
          </ThemeProvider>
        </AppQueryProvider>
      </RootMode>,
    );

    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        stopStartupWatchdog();
        markStartupStage("mounted");
        finishPerformanceTimerAfterPaint(coldStartTimer);
        logger.info("Frontend bootstrap mounted");
      });
    });
  } catch (error) {
    stopStartupWatchdog();
    markStartupStage("failed");
    const message = errorMessage(error);
    const details =
      error instanceof Error ? `${error.message}\n${error.stack ?? ""}` : String(error);
    recordErrorReport("startup.bootstrap", error, {
      fatal: true,
    });
    logger.error(`Frontend bootstrap failed: ${message}`, details);
    renderStartupFallback({
      title: "SiteCMD could not start",
      description:
        "The app failed during startup before the main shell loaded. Reload the app to try again, or reset the saved state if a bad project selection or shell page is trapping startup.",
      details: message,
    });
  }
}

void bootstrap();
