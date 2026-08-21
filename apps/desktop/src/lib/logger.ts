import { logFrontend } from "@/lib/commands";
import { recordErrorReport } from "@/lib/observability";

const MAX_FRONTEND_LOG_CHARS = 2_000;

export function sanitizeFrontendLogText(value: string): string {
  const sanitized = value
    .replace(
      /\bauthorization\s*:\s*bearer\s+[^"',;\s]+|\b(?:api[_-]?key|bearer|token|secret|license[_-]?key|refresh[_-]?token|access[_-]?token)\s*[:=]\s*["']?[^"',;\s]+/gi,
      "[secret]",
    )
    .replace(/\b(?:ghp|github_pat|sk|rk|pk|xox[baprs]|AIza)[A-Za-z0-9_:-]{8,}\b/g, "[secret]")
    .replace(/https?:\/\/[^\s)]+/gi, "[url]")
    .replace(/\b(?:localhost|127\.0\.0\.1)(?::\d+)?\b/gi, "[local-url]")
    .replace(/\/(?:Users|home|var|tmp|private|Volumes)\/[^\s)]+/g, "[path]")
    .replace(/[A-Z]:\\[^\s)]+/g, "[path]")
    .replace(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi, "[email]")
    .trim();

  if (sanitized.length <= MAX_FRONTEND_LOG_CHARS) return sanitized;
  return `${sanitized.slice(0, MAX_FRONTEND_LOG_CHARS)}...[truncated]`;
}

function send(level: string, message: string, context?: string) {
  logFrontend({
    level,
    message: sanitizeFrontendLogText(message),
    context: context ? sanitizeFrontendLogText(context) : undefined,
  }).catch(() => {
    // If the bridge itself fails, there's nothing we can do - avoid infinite loops
  });
}

export const logger = {
  error: (message: string, context?: string) => send("error", message, context),
  warn: (message: string, context?: string) => send("warn", message, context),
  info: (message: string, context?: string) => send("info", message, context),
  debug: (message: string, context?: string) => send("debug", message, context),
};

/** Route global exceptions and unhandled rejections to the Rust log. */
export function installGlobalErrorHandlers() {
  window.addEventListener("error", (event) => {
    const { message, filename, lineno, colno } = event;
    const location = filename ? `${filename}:${lineno}:${colno}` : "unknown";
    recordErrorReport("window.error", event.error ?? message, {
      fatal: true,
      meta: { location },
    });
    logger.error(`Uncaught error: ${message}`, location);
  });

  window.addEventListener("unhandledrejection", (event) => {
    const reason =
      event.reason instanceof Error
        ? `${event.reason.message}\n${event.reason.stack ?? ""}`
        : String(event.reason);
    recordErrorReport("window.unhandledrejection", event.reason, {
      fatal: true,
    });
    logger.error(`Unhandled promise rejection: ${reason}`);
  });

  logger.info("Frontend error handlers installed");
}
