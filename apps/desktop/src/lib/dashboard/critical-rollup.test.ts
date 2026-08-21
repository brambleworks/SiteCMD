import { describe, it, expect } from "vitest";
import { buildCriticalRollup } from "./critical-rollup";

describe("buildCriticalRollup", () => {
  it("returns all zeros when nothing is critical", () => {
    const r = buildCriticalRollup({
      criticalWebIssues: 0,
      criticalCodeIssues: 0,
      securityPatchCount: 0,
    });
    expect(r).toEqual({ total: 0, web: 0, code: 0, securityPatches: 0 });
  });

  it("sums web + code + security patches into total", () => {
    const r = buildCriticalRollup({
      criticalWebIssues: 2,
      criticalCodeIssues: 1,
      securityPatchCount: 3,
    });
    expect(r).toEqual({ total: 6, web: 2, code: 1, securityPatches: 3 });
  });

  it("treats missing fields as zero", () => {
    const r = buildCriticalRollup({ criticalWebIssues: 5 });
    expect(r.total).toBe(5);
    expect(r.code).toBe(0);
    expect(r.securityPatches).toBe(0);
  });
});
