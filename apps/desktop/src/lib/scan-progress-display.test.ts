import { describe, expect, it } from "vitest";
import { getWebScanProgressDetail, getWebScanProgressLabel } from "./scan-progress-display";

describe("scan progress display", () => {
  it("uses human labels for non-counted scan phases", () => {
    const progress = { check_id: "browser-analysis", checks_done: 0, checks_total: 0 };

    expect(getWebScanProgressLabel(progress)).toBe("Running browser metrics");
    expect(getWebScanProgressDetail(progress)).toBe("Running browser metrics");
  });

  it("counts checks once the pipeline reports a total", () => {
    expect(
      getWebScanProgressDetail({ check_id: "security.headers", checks_done: 5, checks_total: 10 }),
    ).toBe("5 of 10 checks");
    expect(getWebScanProgressDetail(null)).toBe("Starting...");
  });
});
