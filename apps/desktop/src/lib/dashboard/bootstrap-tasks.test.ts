import { describe, it, expect } from "vitest";
import { buildBootstrapTasks, type BootstrapInputs } from "./bootstrap-tasks";

const allConfigured: BootstrapInputs = {
  hasProjectFolder: true,
  hasCodeScan: true,
  hasSchedule: true,
  hasAnalytics: true,
  hasUptime: true,
  hasSearch: true,
  hasGithub: true,
  hasReportSchedule: true,
  mcpConfigured: true,
};

describe("buildBootstrapTasks", () => {
  it("returns an empty list when everything is configured", () => {
    expect(buildBootstrapTasks(allConfigured)).toEqual([]);
  });

  it("suggests linking folder when no project folder", () => {
    const tasks = buildBootstrapTasks({
      ...allConfigured,
      hasProjectFolder: false,
      hasCodeScan: false,
    });
    const kinds = tasks.map((t) => t.kind);
    expect(kinds).toContain("code-scan-link");
    expect(kinds).not.toContain("code-scan-run");
  });

  it("suggests running code scan when folder linked but no scan", () => {
    const tasks = buildBootstrapTasks({ ...allConfigured, hasCodeScan: false });
    const kinds = tasks.map((t) => t.kind);
    expect(kinds).toContain("code-scan-run");
    expect(kinds).not.toContain("code-scan-link");
  });

  it("orders code-scan before schedule before analytics", () => {
    const tasks = buildBootstrapTasks({
      ...allConfigured,
      hasProjectFolder: true,
      hasCodeScan: false,
      hasSchedule: false,
      hasAnalytics: false,
    });
    const kinds = tasks.map((t) => t.kind);
    const codeScanIdx = kinds.indexOf("code-scan-run");
    const scheduleIdx = kinds.indexOf("schedule");
    const analyticsIdx = kinds.indexOf("analytics");
    expect(codeScanIdx).toBeLessThan(scheduleIdx);
    expect(scheduleIdx).toBeLessThan(analyticsIdx);
  });

  it("routes the schedule task to the Scanning settings tab, not the run-now scan form", () => {
    const tasks = buildBootstrapTasks({ ...allConfigured, hasSchedule: false });
    const schedule = tasks.find((t) => t.kind === "schedule");
    expect(schedule?.target).toEqual({ type: "nav-settings", tab: "scanning" });
  });

  it("omits tasks for already-configured capabilities", () => {
    const tasks = buildBootstrapTasks({
      ...allConfigured,
      hasSchedule: false,
    });
    expect(tasks.map((t) => t.kind)).toEqual(["schedule"]);
  });

  it("returns tasks ordered by ascending priority", () => {
    const tasks = buildBootstrapTasks({
      ...allConfigured,
      hasCodeScan: false,
      hasAnalytics: false,
      hasUptime: false,
    });
    const priorities = tasks.map((t) => t.priority);
    const sorted = [...priorities].sort((a, b) => a - b);
    expect(priorities).toEqual(sorted);
  });
});
