import { describe, it, expect } from "vitest";
import { deriveSiteVerdict } from "./verdict";
import type { DashboardSnapshotInputs } from "./types";

const base: DashboardSnapshotInputs = {
  criticalWebIssues: 0,
  criticalCodeIssues: 0,
  securityPatchCount: 0,
  highWebIssues: 0,
  deployFailed: false,
  integrationFailureCount: 0,
  staleIntegrationCount: 0,
  searchRegressionNegative: false,
  sslDaysRemaining: 200,
};

describe("deriveSiteVerdict", () => {
  it("returns healthy when nothing is wrong", () => {
    expect(deriveSiteVerdict(base)).toEqual({
      kind: "healthy",
      phrase: "Healthy",
      reasons: [],
    });
  });

  it("returns blocked when a critical web issue exists", () => {
    const v = deriveSiteVerdict({ ...base, criticalWebIssues: 1 });
    expect(v.kind).toBe("blocked");
    expect(v.phrase).toBe("Blocked");
    expect(v.reasons[0]).toMatch(/1 critical web issue/i);
  });

  it("returns blocked when deploy failed", () => {
    const v = deriveSiteVerdict({ ...base, deployFailed: true });
    expect(v.kind).toBe("blocked");
    expect(v.reasons[0]).toMatch(/deploy failed/i);
  });

  it("returns blocked when SSL is under 14 days", () => {
    const v = deriveSiteVerdict({ ...base, sslDaysRemaining: 10 });
    expect(v.kind).toBe("blocked");
    expect(v.reasons[0]).toMatch(/ssl.*10/i);
  });

  it("returns attention when SSL is under 30 days but over 14", () => {
    const v = deriveSiteVerdict({ ...base, sslDaysRemaining: 20 });
    expect(v.kind).toBe("attention");
  });

  it("returns attention when an integration failed", () => {
    const v = deriveSiteVerdict({ ...base, integrationFailureCount: 1 });
    expect(v.kind).toBe("attention");
  });

  it("returns attention when an integration is stale", () => {
    const v = deriveSiteVerdict({ ...base, staleIntegrationCount: 2 });
    expect(v.kind).toBe("attention");
  });

  it("returns attention when search regression is negative", () => {
    const v = deriveSiteVerdict({ ...base, searchRegressionNegative: true });
    expect(v.kind).toBe("attention");
  });

  it("blocked takes precedence over attention", () => {
    const v = deriveSiteVerdict({
      ...base,
      criticalWebIssues: 1,
      integrationFailureCount: 1,
      sslDaysRemaining: 20,
    });
    expect(v.kind).toBe("blocked");
  });

  it("ignores SSL when probe result is null", () => {
    const v = deriveSiteVerdict({ ...base, sslDaysRemaining: null });
    expect(v.kind).toBe("healthy");
  });

  it("caps reasons at 3 entries even when many triggers fire", () => {
    const v = deriveSiteVerdict({
      ...base,
      criticalWebIssues: 5,
      criticalCodeIssues: 2,
      securityPatchCount: 3,
      deployFailed: true,
      sslDaysRemaining: 5,
    });
    expect(v.reasons.length).toBeLessThanOrEqual(3);
  });
});
