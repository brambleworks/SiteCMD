import { clsx, type ClassValue } from "clsx";

export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}

/** Extract hostname from a URL string. Returns empty string if URL is invalid. */
export function getHostname(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return "";
  }
}

/** Extract a display pathname from a URL string without letting malformed URLs crash render paths. */
export function getUrlPathname(url: string | null | undefined, fallback = ""): string {
  if (!url) return fallback;
  try {
    return new URL(url).pathname || "/";
  } catch {
    return fallback;
  }
}

/** Format a URL-like value as the meaningful path when present, otherwise the host. */
export function formatUrlPathOrHost(value: string | null | undefined, fallback = ""): string {
  const trimmed = value?.trim();
  if (!trimmed) return fallback;
  try {
    const parsed = new URL(trimmed);
    return parsed.pathname && parsed.pathname !== "/" ? parsed.pathname : parsed.hostname;
  } catch {
    return trimmed || fallback;
  }
}

/** Format a URL-like value as host plus path while stripping query/fragment secrets. */
export function formatUrlHostPath(value: string | null | undefined, fallback = ""): string {
  const trimmed = value?.trim();
  if (!trimmed) return fallback;
  try {
    const parsed = new URL(trimmed);
    return `${parsed.hostname}${parsed.pathname !== "/" ? parsed.pathname : ""}` || fallback;
  } catch {
    return trimmed.split(/[?#]/, 1)[0] || fallback;
  }
}

/** Format a URL for compact UI labels while preserving path/query context. */
export function formatUrlDisplay(url: string | null | undefined, fallback = ""): string {
  const value = url?.trim();
  if (!value) return fallback;
  return value.replace(/^https?:\/\//i, "").replace(/\/$/, "") || fallback;
}

/** Extract a host label from a URL without throwing during render paths. */
export function formatUrlHost(url: string | null | undefined, fallback = ""): string {
  const value = url?.trim();
  if (!value) return fallback;
  const displayValue = formatUrlDisplay(value, fallback);
  const parseTarget = /^[a-z][a-z\d+.-]*:\/\//i.test(value) ? value : `https://${displayValue}`;
  try {
    return new URL(parseTarget).host || fallback;
  } catch {
    return displayValue.split(/[/?#]/)[0] || fallback;
  }
}
