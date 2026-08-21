import { describe, it, expect, vi } from "vitest";
import { buildSetupRows } from "./setup-rows";
import type { BootstrapTask } from "./types";

const noop = () => {};

const task = (overrides: Partial<BootstrapTask>): BootstrapTask => ({
  kind: "analytics",
  label: "Analytics",
  value: "Connect traffic source",
  target: { type: "nav-settings", tab: "integrations" },
  priority: 30,
  ...overrides,
});

describe("buildSetupRows", () => {
  it("orders rows by priority regardless of input order", () => {
    const rows = buildSetupRows(
      [
        task({ kind: "report", label: "Report", priority: 80 }),
        task({ kind: "analytics", label: "Analytics", priority: 30 }),
        task({ kind: "schedule", label: "Schedule", priority: 20 }),
      ],
      noop,
    );
    expect(rows.map((r) => r.label)).toEqual(["Schedule", "Analytics", "Report"]);
  });

  it("caps at 5 rows even when the task list is long", () => {
    const bootstrap = Array.from({ length: 9 }, (_, i) => task({ label: `B${i}`, priority: i }));
    const rows = buildSetupRows(bootstrap, noop);
    expect(rows.length).toBe(5);
    expect(rows[0].label).toBe("B0");
  });

  it("wires onOpen to the originating task", () => {
    const onOpen = vi.fn();
    const schedule = task({ kind: "schedule", label: "Schedule", priority: 20 });
    const rows = buildSetupRows([schedule], onOpen);
    expect(rows[0].id).toBe("bootstrap:schedule");
    rows[0].onOpen();
    expect(onOpen).toHaveBeenCalledWith(schedule);
  });

  it("returns empty array when everything is set up", () => {
    expect(buildSetupRows([], noop)).toEqual([]);
  });
});
