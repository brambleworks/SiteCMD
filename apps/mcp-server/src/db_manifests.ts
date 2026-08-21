/**
 * Validated loaders for generated MCP parity manifests.
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

// Generated from the Rust candidate table and kept in sync by its parity test.

interface FixLocationCandidate {
  label: string;
  reason: string;
  paths: string[];
}

const __dirname = dirname(fileURLToPath(import.meta.url));

export function parseFixLocationsManifest(value: unknown): Record<string, FixLocationCandidate[]> {
  if (!isRecord(value) || Object.keys(value).length === 0) {
    throw new Error("fix_locations.json must contain at least one check_id mapping");
  }
  for (const [checkId, candidates] of Object.entries(value)) {
    if (!checkId.trim() || !Array.isArray(candidates) || candidates.length === 0) {
      throw new Error(`fix_locations.json has an invalid candidate list for '${checkId}'`);
    }
    for (const [index, candidate] of candidates.entries()) {
      if (
        !isRecord(candidate) ||
        typeof candidate.label !== "string" ||
        !candidate.label.trim() ||
        typeof candidate.reason !== "string" ||
        !candidate.reason.trim() ||
        !Array.isArray(candidate.paths) ||
        candidate.paths.length === 0 ||
        candidate.paths.some((path) => typeof path !== "string" || !path.trim())
      ) {
        throw new Error(
          `fix_locations.json has invalid label, reason, or paths fields for '${checkId}' candidate ${index}`,
        );
      }
    }
  }
  return value as unknown as Record<string, FixLocationCandidate[]>;
}

function readFixLocationsJson(): Record<string, FixLocationCandidate[]> {
  const path = join(__dirname, "fix_locations.json");
  try {
    return parseFixLocationsManifest(JSON.parse(readFileSync(path, "utf8")) as unknown);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Unable to load generated fix_locations.json at ${path}: ${detail}`, {
      cause: error,
    });
  }
}

const FIX_LOCATIONS: Record<string, FixLocationCandidate[]> = readFixLocationsJson();

export function getFixLocationsForCheckId(checkId: string): FixLocationCandidate[] {
  return FIX_LOCATIONS[checkId] ?? [];
}

// Generated from Rust scoring constants and verified through the bundled grid.

interface ImpactScoreManifest {
  severity_penalties: Record<string, number>;
  default_severity_penalty: number;
  category_weights: Record<string, number>;
  default_category_weight: number;
  base_multiplier: number;
  extra_source_bonus_per_source: number;
  grid: { severity: string; category: string; source_count: number; score: number }[];
}

function isFiniteNonnegativeNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

function isNonnegativeNumberRecord(value: unknown): value is Record<string, number> {
  return (
    isRecord(value) &&
    Object.keys(value).length > 0 &&
    Object.values(value).every(isFiniteNonnegativeNumber)
  );
}

export function parseImpactScoreManifest(value: unknown): ImpactScoreManifest {
  if (
    !isRecord(value) ||
    !isNonnegativeNumberRecord(value.severity_penalties) ||
    !isFiniteNonnegativeNumber(value.default_severity_penalty) ||
    !isNonnegativeNumberRecord(value.category_weights) ||
    !isFiniteNonnegativeNumber(value.default_category_weight) ||
    !isFiniteNonnegativeNumber(value.base_multiplier) ||
    !isFiniteNonnegativeNumber(value.extra_source_bonus_per_source) ||
    !Array.isArray(value.grid) ||
    value.grid.length === 0
  ) {
    throw new Error("impact_score.json is missing required weights, constants, or grid fields");
  }

  const manifest = value as unknown as ImpactScoreManifest;
  for (const [index, row] of manifest.grid.entries()) {
    if (
      !isRecord(row) ||
      typeof row.severity !== "string" ||
      !row.severity.trim() ||
      typeof row.category !== "string" ||
      !row.category.trim() ||
      !Number.isInteger(row.source_count) ||
      row.source_count < 1 ||
      !isFiniteNonnegativeNumber(row.score)
    ) {
      throw new Error(`impact_score.json has an invalid grid row at index ${index}`);
    }
    const severityPenalty =
      manifest.severity_penalties[row.severity] ?? manifest.default_severity_penalty;
    const categoryWeight =
      manifest.category_weights[row.category] ?? manifest.default_category_weight;
    const expected =
      severityPenalty * categoryWeight * manifest.base_multiplier +
      Math.max(0, row.source_count - 1) * manifest.extra_source_bonus_per_source;
    if (Math.abs(expected - row.score) >= 1e-9) {
      throw new Error(`impact_score.json grid row ${index} disagrees with its declared formula`);
    }
  }
  return manifest;
}

function readImpactScoreJson(): ImpactScoreManifest {
  const path = join(__dirname, "impact_score.json");
  try {
    return parseImpactScoreManifest(JSON.parse(readFileSync(path, "utf8")) as unknown);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Unable to load generated impact_score.json at ${path}: ${detail}`, {
      cause: error,
    });
  }
}

const IMPACT_SCORE: ImpactScoreManifest = readImpactScoreJson();

export function computeImpactScore(
  severity: string,
  category: string,
  sourceCount: number,
): number {
  const sevPenalty =
    IMPACT_SCORE.severity_penalties[severity] ?? IMPACT_SCORE.default_severity_penalty;
  const catWeight = IMPACT_SCORE.category_weights[category] ?? IMPACT_SCORE.default_category_weight;
  const base = sevPenalty * catWeight * IMPACT_SCORE.base_multiplier;
  return base + Math.max(0, sourceCount - 1) * IMPACT_SCORE.extra_source_bonus_per_source;
}

/** Exposes the generated grid so the test suite can replay it. */
export function __test_impactScoreGrid(): ImpactScoreManifest["grid"] {
  return IMPACT_SCORE.grid;
}

// Generated from Rust licensing constants; invalid data indicates a broken install.

interface LicenseConstants {
  offline_grace_period_secs: number;
}

export function parseLicenseConstantsManifest(value: unknown): LicenseConstants {
  if (
    !isRecord(value) ||
    !Number.isInteger(value.offline_grace_period_secs) ||
    (value.offline_grace_period_secs as number) < 0
  ) {
    throw new Error("license_constants.json is missing the offline grace period");
  }
  if ("features" in value || "free_history_limit" in value) {
    throw new Error(
      "license_constants.json carries a retired feature table; regenerate it with cargo test",
    );
  }
  return value as unknown as LicenseConstants;
}

function readLicenseConstantsJson(): LicenseConstants {
  const path = join(__dirname, "license_constants.json");
  try {
    return parseLicenseConstantsManifest(JSON.parse(readFileSync(path, "utf8")) as unknown);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Unable to load generated license_constants.json at ${path}: ${detail}`, {
      cause: error,
    });
  }
}

const LICENSE_CONSTANTS: LicenseConstants = readLicenseConstantsJson();

export const OFFLINE_GRACE_PERIOD_SECS = LICENSE_CONSTANTS.offline_grace_period_secs;
