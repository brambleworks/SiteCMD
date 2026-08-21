const DEFAULT_TIMEOUT_MS = 15000;
const DEFAULT_POLL_MS = 350;
const ROOT_MARKER_PATTERN = /id\s*=\s*["']root["']/i;

export function attachRiskAcknowledged(environment = process.env) {
  return environment.SITECMD_ALLOW_PRIVILEGED_ATTACH === "1";
}

export function isHealthyDevServerResponse({ ok, body, contentType = "" }) {
  if (!ok) return false;

  const normalizedBody = body || "";
  const normalizedContentType = contentType.toLowerCase();

  if (!normalizedContentType.includes("text/html")) return false;
  if (!ROOT_MARKER_PATTERN.test(normalizedBody)) return false;

  return (
    normalizedBody.includes("/@vite/client") ||
    normalizedBody.includes("/src/main.tsx") ||
    normalizedBody.includes("/src/main.ts")
  );
}

async function fetchDevServerResponse(url, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), options.requestTimeoutMs ?? 4000);

  try {
    const response = await fetch(url, {
      signal: controller.signal,
      headers: { accept: "text/html" },
    });
    const body = await response.text();
    return {
      ok: response.ok,
      body,
      contentType: response.headers.get("content-type") ?? "",
    };
  } catch {
    return {
      ok: false,
      body: "",
      contentType: "",
    };
  } finally {
    clearTimeout(timeout);
  }
}

export async function waitForHealthyDevServer(url, options = {}) {
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const pollMs = options.pollMs ?? DEFAULT_POLL_MS;
  const startedAt = Date.now();
  const fetchResponse = options.fetchResponse ?? fetchDevServerResponse;

  while (Date.now() - startedAt < timeoutMs) {
    const result = await fetchResponse(url, options);
    if (isHealthyDevServerResponse(result)) {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, pollMs));
  }

  return false;
}

export function buildAttachPreflightFailureMessage(url) {
  return [
    `SiteCMD attach preflight failed: ${url} is not serving a healthy Vite app yet.`,
    "Start the web app first with `pnpm dev` or `pnpm dev:tauri`, wait for the Vite URL to load, then run `pnpm tauri:dev:attach`.",
    "If you still get a white window, close any older SiteCMD instance before attaching again.",
  ];
}
