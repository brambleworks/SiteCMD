export type ProjectEnvironment = "production" | "staging" | "development" | "local";

// Match the Rust write-boundary URL normalization.
function lowercaseUrlOrigin(url: string): string {
  const schemeEnd = url.indexOf("://");
  if (schemeEnd === -1) return url;
  const afterScheme = schemeEnd + 3;
  const rest = url.slice(afterScheme);
  const separatorIndex = rest.search(/[/?#]/);
  const hostEnd = separatorIndex === -1 ? rest.length : separatorIndex;
  return (
    url.slice(0, afterScheme).toLowerCase() +
    rest.slice(0, hostEnd).toLowerCase() +
    rest.slice(hostEnd)
  );
}

export function normalizeProjectUrlInput(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  const withScheme = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
  return lowercaseUrlOrigin(withScheme.replace(/\/+$/, ""));
}

function isLoopbackHost(hostname: string): boolean {
  const lower = hostname.toLowerCase();
  return (
    lower === "localhost" ||
    lower === "127.0.0.1" ||
    lower === "0.0.0.0" ||
    lower === "::1" ||
    lower === "[::1]"
  );
}

/** Return whether a detected project URL points to this machine. */
export function isLoopbackProjectUrl(raw: string): boolean {
  const normalized = normalizeProjectUrlInput(raw);
  if (!normalized) return false;
  try {
    return isLoopbackHost(new URL(normalized).hostname);
  } catch {
    return false;
  }
}

export function getProjectUrlIdentityKey(raw: string): string {
  const normalized = normalizeProjectUrlInput(raw);
  if (!normalized) return "";

  try {
    const parsed = new URL(normalized);
    const host = isLoopbackHost(parsed.hostname) ? "localhost" : parsed.hostname.toLowerCase();
    const path = parsed.pathname.replace(/\/+$/, "") || "/";
    const port = parsed.port ? `:${parsed.port}` : "";
    return `${parsed.protocol.toLowerCase()}//${host}${port}${path}${parsed.search}`;
  } catch {
    return normalized.toLowerCase().replace(/\/+$/, "");
  }
}

// Hostnames local dev environments publish for a site served from this
// machine: DDEV, Lando, Docksal, and the RFC 6761 `.test` TLD used by
// Valet/Herd and hand-rolled Docker setups. They resolve to loopback but do
// not read as local, so a detected DDEV URL would otherwise land on
// Production. Mirrored by LOCAL_DEV_HOST_SUFFIXES in
// src-tauri/src/core/localhost.rs.
const LOCAL_DEV_HOST_SUFFIXES = [".ddev.site", ".lndo.site", ".docksal.site", ".test"];

export function inferProjectEnvironmentFromUrl(raw: string): ProjectEnvironment {
  const normalized = normalizeProjectUrlInput(raw);
  if (!normalized) return "production";

  try {
    const { hostname } = new URL(normalized);
    const lower = hostname.toLowerCase();

    if (
      lower === "localhost" ||
      lower === "127.0.0.1" ||
      lower === "0.0.0.0" ||
      lower === "::1" ||
      lower.endsWith(".local") ||
      lower.includes("localhost") ||
      LOCAL_DEV_HOST_SUFFIXES.some((suffix) => lower.endsWith(suffix))
    ) {
      return "local";
    }

    if (
      lower.startsWith("dev.") ||
      lower.includes(".dev.") ||
      lower.includes("-dev.") ||
      lower.includes("development")
    ) {
      return "development";
    }

    if (
      lower.startsWith("staging.") ||
      lower.includes(".staging.") ||
      lower.includes("-staging.") ||
      lower.includes("stage") ||
      lower.includes("preview") ||
      lower.includes("qa") ||
      lower.endsWith(".vercel.app") ||
      lower.endsWith(".netlify.app") ||
      lower.endsWith(".onrender.com")
    ) {
      return "staging";
    }
  } catch {
    return "production";
  }

  return "production";
}

export function resolveProjectEnvironmentForUrl(
  raw: string,
  provided?: ProjectEnvironment,
): ProjectEnvironment {
  const inferred = inferProjectEnvironmentFromUrl(raw);
  if (!provided) return inferred;
  if (inferred === "local") return "local";
  if (provided === "local") return inferred;
  if (provided === "production" && inferred !== "production") return inferred;
  return provided;
}
