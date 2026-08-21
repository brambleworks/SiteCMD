import type { AppTarget, AppTargetPage } from "@/lib/app-targets";
import {
  normalizeAppTargetPage,
  normalizeHttpTargetUrl,
  withNormalizedTarget,
} from "@/lib/app-targets";

const APP_PAGES = new Set<AppTargetPage>([
  "dashboard",
  "updates",
  "search-console",
  "analytics",
  "integrations",
  "events",
  "deploys",
  "issues",
  "alerts",
  "sites",
  "settings",
]);
const SHORT_TEXT_PARAM_MAX_LENGTH = 200;
const FILE_PATH_PARAM_MAX_LENGTH = 1_000;

function parsePage(value: string | null): AppTargetPage | null {
  const normalized = normalizeAppTargetPage(value);
  if (!normalized) return null;
  return APP_PAGES.has(normalized) ? normalized : null;
}

function parseInteger(value: string | null): number | null {
  if (!value || !/^[1-9]\d*$/.test(value)) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function parseBoolean(value: string | null): boolean {
  return value === "1" || value === "true";
}

function hasControlCharacter(value: string): boolean {
  return value.split("").some((char) => {
    const code = char.charCodeAt(0);
    return code <= 31 || code === 127;
  });
}

function parseTextParam(
  value: string | null,
  maxLength = SHORT_TEXT_PARAM_MAX_LENGTH,
): string | null {
  if (!value || value.length > maxLength || hasControlCharacter(value)) return null;
  return value;
}

// Must match CONNECTED_ID_MAX_LENGTH in desktop_deep_links.rs.
const CONNECTED_ID_MAX_LENGTH = 128;
const CONNECTED_ID_PATTERN = /^[A-Za-z0-9_-]+$/;

const CONNECTED_SETTINGS_SECTIONS = new Set(["notifications", "admins"]);

export const CONNECTED_ALERT_UNAVAILABLE = "connected-alert-unavailable";
export const CONNECTED_LINK_UNKNOWN = "connected-link-unknown";

function isBoundedConnectedId(value: string): boolean {
  return value.length <= CONNECTED_ID_MAX_LENGTH && CONNECTED_ID_PATTERN.test(value);
}

// A bare connected host is an OAuth return; all other ids are bounded before lookup.
function parseConnectedDeepLink(parsedUrl: URL): AppTarget | null {
  const segments = parsedUrl.pathname.split("/").filter(Boolean);
  if (segments.length === 0) return null;

  const notFound = (reason: string): AppTarget => withNormalizedTarget({ page: "alerts", reason });

  if (segments[0] === "alerts") {
    const alertId = segments.length === 2 ? segments[1] : null;
    if (!alertId || !isBoundedConnectedId(alertId)) {
      return notFound(CONNECTED_ALERT_UNAVAILABLE);
    }
    return withNormalizedTarget({ page: "alerts", itemId: alertId });
  }

  if (segments.length === 2 && segments[0] === "settings") {
    if (!CONNECTED_SETTINGS_SECTIONS.has(segments[1])) {
      return notFound(CONNECTED_LINK_UNKNOWN);
    }
    return withNormalizedTarget({ page: "settings", focus: "connected" });
  }

  return notFound(CONNECTED_LINK_UNKNOWN);
}

export function parseDeepLinkUrl(rawUrl: string): AppTarget | null {
  let parsedUrl: URL;
  try {
    parsedUrl = new URL(rawUrl);
  } catch {
    return null;
  }

  if (parsedUrl.protocol !== "sitecmd:") return null;
  if (parsedUrl.hostname === "connected") return parseConnectedDeepLink(parsedUrl);

  const pathSegments = parsedUrl.pathname.split("/").filter(Boolean);
  let page = parsePage(parsedUrl.searchParams.get("page"));
  let projectId = parseInteger(parsedUrl.searchParams.get("projectId"));

  if (!page) {
    if (parsedUrl.hostname === "open") {
      page = parsePage(parsedUrl.searchParams.get("page"));
    } else if (parsedUrl.hostname === "project") {
      projectId = projectId ?? parseInteger(pathSegments[0] ?? null);
      page = parsePage(pathSegments[1] ?? null);
    } else {
      page = parsePage(parsedUrl.hostname || pathSegments[0] || null);
    }
  }

  if (!page) return null;

  return withNormalizedTarget({
    page,
    projectId,
    url: normalizeHttpTargetUrl(parsedUrl.searchParams.get("url")),
    scanId: parseInteger(parsedUrl.searchParams.get("scanId")),
    sessionId: parseInteger(parsedUrl.searchParams.get("sessionId")),
    scanKind:
      parsedUrl.searchParams.get("scanKind") === "code"
        ? "code"
        : parsedUrl.searchParams.get("scanKind") === "site"
          ? "site"
          : null,
    focus: parseTextParam(parsedUrl.searchParams.get("focus")),
    itemId: parseTextParam(parsedUrl.searchParams.get("itemId")),
    promptId: parseTextParam(parsedUrl.searchParams.get("promptId")),
    lane:
      parsedUrl.searchParams.get("lane") === "pending-verification" ? "pending-verification" : null,
    reason: parseTextParam(parsedUrl.searchParams.get("reason")),
    filePath: parseTextParam(parsedUrl.searchParams.get("filePath"), FILE_PATH_PARAM_MAX_LENGTH),
    restoreScan: parseBoolean(parsedUrl.searchParams.get("restoreScan")),
  });
}

// Must match the Rust decoder and AccountSettings input cap.
const LICENSE_KEY_MAX_LENGTH = 256;

/** Parse a startup activation captured before the webview listener mounts. */
export function parseActivateDeepLink(rawUrl: string): string | null {
  let parsedUrl: URL;
  try {
    parsedUrl = new URL(rawUrl);
  } catch {
    return null;
  }
  if (parsedUrl.protocol !== "sitecmd:" || parsedUrl.hostname !== "activate") return null;
  const key = parsedUrl.searchParams.get("key")?.trim();
  if (!key || key.length > LICENSE_KEY_MAX_LENGTH) return null;
  return key;
}

/** Return the latest activation key from startup URLs. */
export function latestActivateDeepLinkKey(
  urls: readonly string[] | null | undefined,
): string | null {
  if (!urls) return null;
  for (let index = urls.length - 1; index >= 0; index -= 1) {
    const key = parseActivateDeepLink(urls[index]);
    if (key) return key;
  }
  return null;
}

export function buildDeepLinkUrl(target: AppTarget): string {
  const normalized = withNormalizedTarget(target);
  const url = new URL("sitecmd://open");
  url.searchParams.set("page", normalized.page);
  if (normalized.projectId != null) url.searchParams.set("projectId", String(normalized.projectId));
  if (normalized.url) url.searchParams.set("url", normalized.url);
  if (normalized.scanId != null) url.searchParams.set("scanId", String(normalized.scanId));
  if (normalized.sessionId != null) url.searchParams.set("sessionId", String(normalized.sessionId));
  if (normalized.scanKind) url.searchParams.set("scanKind", normalized.scanKind);
  if (normalized.focus) url.searchParams.set("focus", normalized.focus);
  if (normalized.itemId) url.searchParams.set("itemId", normalized.itemId);
  if (normalized.promptId) url.searchParams.set("promptId", normalized.promptId);
  if (normalized.lane) url.searchParams.set("lane", normalized.lane);
  if (normalized.reason) url.searchParams.set("reason", normalized.reason);
  if (normalized.filePath) url.searchParams.set("filePath", normalized.filePath);
  if (normalized.restoreScan) url.searchParams.set("restoreScan", "1");
  return url.toString();
}
