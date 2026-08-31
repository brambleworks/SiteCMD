import path from "node:path";
import { fileURLToPath } from "node:url";

const INSTALL_LOCATION_PATTERN = /^[ \t]*Install location:[ \t]*(.+)$/gm;

/**
 * Classify a socket bind failure so the gate reports the remediation that
 * actually applies: a held port, a denied bind, or an address the host cannot
 * offer at all.
 */
export function classifyBindError(error) {
  if (error?.code === "EADDRINUSE") return "occupied";
  if (error?.code === "EACCES" || error?.code === "EPERM") return "denied";
  return "unavailable";
}

/** Resolve the repository root without leaving URL-encoded path segments intact. */
export function resolveRepositoryRoot(moduleUrl) {
  return path.resolve(path.dirname(fileURLToPath(moduleUrl)), "../..");
}

/**
 * Browser cache paths `playwright install --dry-run` says are required.
 * @param {string} dryRunOutput stdout of `playwright install <browser> --dry-run`
 * @returns {string[]}
 */
export function parseBrowserInstallLocations(dryRunOutput) {
  return [...dryRunOutput.matchAll(INSTALL_LOCATION_PATTERN)].map((match) => match[1].trim());
}

/**
 * Return absent browser paths, or `null` when Playwright's output cannot be parsed.
 * @param {string} dryRunOutput
 * @param {(path: string) => boolean} exists
 * @returns {string[] | null}
 */
export function missingBrowserPaths(dryRunOutput, exists) {
  const locations = parseBrowserInstallLocations(dryRunOutput);
  if (locations.length === 0) return null;
  return locations.filter((location) => !exists(location));
}
