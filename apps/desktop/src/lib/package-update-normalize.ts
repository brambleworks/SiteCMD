import type { PackageUpdate, UpdateReport } from "@/lib/types";
import { isJsonRecord } from "@/lib/json-record";

/** Normalize live and persisted package data to the current runtime contract. */

/** Strings only - a persisted array can hold anything. */
function stringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((entry): entry is string => typeof entry === "string");
}

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function boolOr(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

/** Parses one package update, returning null when required fields are invalid. */
export function normalizePackageUpdate(value: unknown): PackageUpdate | null {
  if (!isJsonRecord(value)) return null;
  if (
    typeof value.ecosystem !== "string" ||
    typeof value.name !== "string" ||
    typeof value.currentVersion !== "string" ||
    typeof value.latestVersion !== "string" ||
    typeof value.updateType !== "string" ||
    typeof value.isSecurity !== "boolean"
  ) {
    return null;
  }
  const legacyNoFix = value.isSecurity && value.latestVersion === "no fix available";
  const advisoryFixedVersion = value.isSecurity
    ? stringOrNull(value.advisoryFixedVersion)?.trim() || null
    : null;
  return {
    name: value.name,
    currentVersion: value.currentVersion,
    latestVersion: legacyNoFix ? value.currentVersion : value.latestVersion,
    ecosystem: value.ecosystem as PackageUpdate["ecosystem"],
    updateType: value.updateType as PackageUpdate["updateType"],
    isSecurity: value.isSecurity,
    advisorySeverity: stringOrNull(value.advisorySeverity),
    advisoryUrl: stringOrNull(value.advisoryUrl),
    ...(advisoryFixedVersion ? { advisoryFixedVersion } : {}),
    source: typeof value.source === "string" ? value.source : "unknown",
    isDev: boolOr(value.isDev, false),
    isDeprecated: boolOr(value.isDeprecated, false),
    deprecationMessage: stringOrNull(value.deprecationMessage),
    currentVersionDeprecated: boolOr(value.currentVersionDeprecated, false),
    isStale: boolOr(value.isStale, false),
    lastPublished: stringOrNull(value.lastPublished),
    workspaceMembers: stringArray(value.workspaceMembers),
  };
}

/** One installed package from untrusted JSON, or null when unidentifiable. */
function normalizeInstalledPackage(value: unknown): UpdateReport["packages"][number] | null {
  if (!isJsonRecord(value)) return null;
  if (
    typeof value.name !== "string" ||
    typeof value.version !== "string" ||
    typeof value.ecosystem !== "string"
  ) {
    return null;
  }
  return {
    name: value.name,
    version: value.version,
    ecosystem: value.ecosystem as UpdateReport["packages"][number]["ecosystem"],
    source: typeof value.source === "string" ? value.source : "unknown",
    isDev: boolOr(value.isDev, false),
    workspaceMembers: stringArray(value.workspaceMembers),
  };
}

/** Normalize untrusted JSON into a complete, possibly empty update report. */
export function normalizeUpdateReport(value: unknown): UpdateReport {
  const record = isJsonRecord(value) ? value : {};
  const packages = Array.isArray(record.packages)
    ? record.packages.map(normalizeInstalledPackage).filter((pkg) => pkg !== null)
    : [];
  const updates = Array.isArray(record.updates)
    ? record.updates.map(normalizePackageUpdate).filter((update) => update !== null)
    : [];
  return {
    packages,
    updates,
    ecosystemsDetected: Array.isArray(record.ecosystemsDetected)
      ? (record.ecosystemsDetected as UpdateReport["ecosystemsDetected"])
      : [],
    scanDurationMs: typeof record.scanDurationMs === "number" ? record.scanDurationMs : 0,
  };
}
