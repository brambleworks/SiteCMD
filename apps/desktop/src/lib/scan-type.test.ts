import { describe, expect, it } from "vitest";

import manifest from "@/generated/scan_type.json";
import type { ScanType, ScheduledScanType } from "@/lib/types";

const SCHEDULED_UNION_MEMBERS: Record<ScheduledScanType, true> = {
  health: true,
  security: true,
  accessibility: true,
  polish: true,
  code: true,
  full: true,
};

const WEB_UNION_MEMBERS: Record<ScanType, true> = {
  health: true,
  security: true,
  accessibility: true,
  polish: true,
};

describe("scan type vocabulary parity", () => {
  it("matches the generated Rust scheduled vocabulary exactly", () => {
    expect(Object.keys(SCHEDULED_UNION_MEMBERS).sort()).toEqual(
      [...manifest.scheduled_scan_types].sort(),
    );
  });

  it("matches the generated Rust web-scan vocabulary exactly", () => {
    expect(Object.keys(WEB_UNION_MEMBERS).sort()).toEqual([...manifest.scan_types].sort());
  });

  it("keeps the web subset a strict subset of the scheduled vocabulary", () => {
    const scheduled = new Set(manifest.scheduled_scan_types);
    for (const scanType of manifest.scan_types) {
      expect(scheduled.has(scanType), `${scanType} must be schedulable too`).toBe(true);
    }
    expect(manifest.scheduled_scan_types).toContain("code");
    expect(manifest.scan_types).not.toContain("code");
  });
});
