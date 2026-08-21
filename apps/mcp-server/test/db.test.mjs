import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  __test_impactScoreGrid,
  computeImpactScore,
  parseFixLocationsManifest,
  parseImpactScoreManifest,
  parseLicenseConstantsManifest,
  sanitizeHistoryLimit,
} from "../dist/db.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

test("the licensing manifest carries timing windows and no feature table", () => {
  const manifest = JSON.parse(
    readFileSync(join(__dirname, "..", "dist", "license_constants.json"), "utf8"),
  );
  assert.ok(Number.isInteger(manifest.offline_grace_period_secs));
  assert.equal("features" in manifest, false, "the feature table stays retired");
  assert.equal("free_history_limit" in manifest, false, "the history cap stays retired");
});

test("sanitizeHistoryLimit bounds query size and takes no tier", () => {
  assert.equal(sanitizeHistoryLimit.length, 1, "no tier parameter survives");
  assert.equal(sanitizeHistoryLimit(50), 50);
  assert.equal(sanitizeHistoryLimit(500), 100);
});

test("computeImpactScore reproduces every generated Rust grid row", () => {
  const grid = __test_impactScoreGrid();
  assert.ok(grid.length > 0, "impact_score.json grid must be present in dist");
  for (const row of grid) {
    const actual = computeImpactScore(row.severity, row.category, row.source_count);
    assert.ok(
      Math.abs(actual - row.score) < 1e-9,
      `impact score drift for (${row.severity}, ${row.category}, ${row.source_count}): TS ${actual} vs Rust ${row.score}`,
    );
  }
});

test("all generated DB manifests reject malformed nested fields instead of degrading", () => {
  const fixLocations = JSON.parse(
    readFileSync(join(__dirname, "..", "dist", "fix_locations.json"), "utf8"),
  );
  const impactScore = JSON.parse(
    readFileSync(join(__dirname, "..", "dist", "impact_score.json"), "utf8"),
  );
  const licenseConstants = JSON.parse(
    readFileSync(join(__dirname, "..", "dist", "license_constants.json"), "utf8"),
  );

  assert.doesNotThrow(() => parseFixLocationsManifest(fixLocations));
  assert.doesNotThrow(() => parseImpactScoreManifest(impactScore));
  assert.doesNotThrow(() => parseLicenseConstantsManifest(licenseConstants));

  const malformedLocations = structuredClone(fixLocations);
  malformedLocations[Object.keys(malformedLocations)[0]][0].paths[0] = "";
  assert.throws(() => parseFixLocationsManifest(malformedLocations), /fix_locations.*paths/i);

  const malformedImpact = structuredClone(impactScore);
  malformedImpact.grid[0].score = "375";
  assert.throws(() => parseImpactScoreManifest(malformedImpact), /impact_score.*grid/i);

  const malformedLicense = structuredClone(licenseConstants);
  malformedLicense.offline_grace_period_secs = "later";
  assert.throws(() => parseLicenseConstantsManifest(malformedLicense), /license_constants.*grace/i);

  const resurrectedGate = structuredClone(licenseConstants);
  resurrectedGate.features = { ai_fixes: { label: "AI Fixes", min_tier: "core" } };
  assert.throws(() => parseLicenseConstantsManifest(resurrectedGate), /retired feature table/i);
});
