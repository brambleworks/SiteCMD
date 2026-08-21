import { describe, expect, it } from "vitest";

import { hasSetupError, isIntegrationActive } from "./integration-connection-status";
import type { IntegrationData } from "./integration-services";

function liveData(type: string, error: string | null): IntegrationData {
  return {
    integrationType: type,
    data: {},
    fetchedAt: "2026-04-13T12:00:00Z",
    error,
  };
}

describe("isIntegrationActive", () => {
  it("is false when the integration is not configured", () => {
    expect(isIntegrationActive("plausible", false, undefined)).toBe(false);
    expect(isIntegrationActive("plausible", false, liveData("plausible", null))).toBe(false);
  });

  it("stays connected while live data has not arrived yet", () => {
    expect(isIntegrationActive("plausible", true, undefined)).toBe(true);
  });

  it("stays connected on fetch errors for services without live verification", () => {
    expect(isIntegrationActive("github", true, liveData("github", "HTTP 401"))).toBe(true);
  });

  it("drops to disconnected when a live-verified service reports an error", () => {
    expect(isIntegrationActive("plausible", true, liveData("plausible", "HTTP 404"))).toBe(false);
  });

  it("is connected when a live-verified service returns clean data", () => {
    expect(isIntegrationActive("plausible", true, liveData("plausible", null))).toBe(true);
  });
});

describe("hasSetupError", () => {
  it("flags configured live-verified services whose last fetch errored", () => {
    expect(hasSetupError("plausible", true, liveData("plausible", "HTTP 404"))).toBe(true);
  });

  it("does not flag unconfigured services", () => {
    expect(hasSetupError("plausible", false, liveData("plausible", "HTTP 404"))).toBe(false);
  });

  it("does not flag services without live verification", () => {
    expect(hasSetupError("github", true, liveData("github", "HTTP 401"))).toBe(false);
  });

  it("does not flag clean or missing live data", () => {
    expect(hasSetupError("plausible", true, liveData("plausible", null))).toBe(false);
    expect(hasSetupError("plausible", true, undefined)).toBe(false);
  });
});
