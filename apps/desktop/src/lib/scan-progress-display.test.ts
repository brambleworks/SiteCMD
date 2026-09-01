import { describe, expect, it } from "vitest";
import {
  getMultiScanOverallPercent,
  getWebScanProgressDetail,
  getWebScanProgressInline,
  getWebScanProgressLabel,
  getWebScanProgressPercent,
} from "./scan-progress-display";

describe("scan progress display", () => {
  it("maps counted checks into the weighted pre-browser range", () => {
    expect(
      getWebScanProgressPercent({
        check_id: "security.headers",
        status: "complete",
        checks_done: 5,
        checks_total: 10,
      }),
    ).toBe(39);
    expect(
      getWebScanProgressPercent({
        check_id: "security.headers",
        status: "complete",
        checks_done: 10,
        checks_total: 10,
      }),
    ).toBe(70);
  });

  it("does not reset progress for phase events with no check total", () => {
    expect(
      getWebScanProgressPercent({
        check_id: "fetch",
        status: "running",
        checks_done: 0,
        checks_total: 0,
      }),
    ).toBe(4);
    expect(
      getWebScanProgressPercent({
        check_id: "polish-css",
        status: "running",
        checks_done: 0,
        checks_total: 0,
      }),
    ).toBe(70);
    expect(
      getWebScanProgressPercent({
        check_id: "browser-analysis",
        status: "running",
        checks_done: 0,
        checks_total: 0,
      }),
    ).toBe(75);
  });

  it("uses human labels for non-counted scan phases", () => {
    const progress = {
      check_id: "browser-analysis",
      status: "running",
      checks_done: 0,
      checks_total: 0,
    };

    expect(getWebScanProgressLabel(progress)).toBe("Running browser metrics");
    expect(getWebScanProgressDetail(progress)).toBe("Running browser metrics");
    expect(getWebScanProgressInline(progress)).toBe("Running browser metrics • 75%");
  });

  it("maps per-page progress onto one monotonic multi-page percentage", () => {
    expect(
      getMultiScanOverallPercent({ page_index: 0, page_count: 2, page_status: "complete" }, 100),
    ).toBe(50);
    expect(
      getMultiScanOverallPercent({ page_index: 1, page_count: 2, page_status: "scanning" }, 8),
    ).toBe(54);
    expect(
      getMultiScanOverallPercent({ page_index: 1, page_count: 2, page_status: "complete" }, 96),
    ).toBe(100);
  });
});
