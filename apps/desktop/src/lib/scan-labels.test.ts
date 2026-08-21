import { describe, expect, it } from "vitest";
import { getScanArtifactLabel, SCAN_LABELS } from "@/lib/scan-labels";

describe("scan labels", () => {
  it("keeps scan artifact labels in one place", () => {
    expect(SCAN_LABELS.full).toBe("Full Scan");
    expect(getScanArtifactLabel("health")).toBe("Web Scan");
    expect(getScanArtifactLabel("health", { includeHealthSubtype: true })).toBe("Web Scan · Full");
    expect(getScanArtifactLabel("security")).toBe("Web Scan · Security");
    expect(getScanArtifactLabel("accessibility")).toBe("Web Scan · Accessibility");
    expect(getScanArtifactLabel("polish")).toBe("Web Scan · Polish");
    expect(getScanArtifactLabel("code")).toBe("Code Scan");
    expect(getScanArtifactLabel("session")).toBe("Multi-page Web Scan");
  });

  it("falls back to raw labels for future scan families instead of hiding them", () => {
    expect(getScanArtifactLabel("integration")).toBe("integration");
    expect(getScanArtifactLabel(null)).toBe("Web Scan");
  });
});
