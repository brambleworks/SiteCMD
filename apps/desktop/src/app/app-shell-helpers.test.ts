import { describe, expect, it, vi } from "vitest";
import { createProjectDeletedHandler, shouldShowTelemetryConsentPrompt } from "./app-shell-helpers";

describe("createProjectDeletedHandler", () => {
  it("refreshes projects, then lands on the dashboard so the switch is visible", async () => {
    const order: string[] = [];
    const refreshProjects = vi.fn(async () => {
      order.push("refresh");
    });
    const navigateTo = vi.fn((page: "dashboard") => {
      order.push(`navigate:${page}`);
    });

    await createProjectDeletedHandler({ refreshProjects, navigateTo })();

    expect(order).toEqual(["refresh", "navigate:dashboard"]);
  });
});

describe("shouldShowTelemetryConsentPrompt", () => {
  const ready = {
    hasCompletedFirstScan: true,
    projectCount: 1,
    showScanSummary: false,
    showFirstRunWalkthrough: false,
  };

  it("waits for the first scan and at least one project", () => {
    expect(shouldShowTelemetryConsentPrompt({ ...ready, hasCompletedFirstScan: false })).toBe(
      false,
    );
    expect(shouldShowTelemetryConsentPrompt({ ...ready, projectCount: 0 })).toBe(false);
    expect(shouldShowTelemetryConsentPrompt(ready)).toBe(true);
  });

  it("never covers the scan summary or the first-run walkthrough", () => {
    expect(shouldShowTelemetryConsentPrompt({ ...ready, showScanSummary: true })).toBe(false);
    expect(shouldShowTelemetryConsentPrompt({ ...ready, showFirstRunWalkthrough: true })).toBe(
      false,
    );
  });
});
