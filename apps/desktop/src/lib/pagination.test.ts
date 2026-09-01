import { describe, expect, it } from "vitest";
import { buildPagerItems } from "@/lib/pagination";

describe("buildPagerItems", () => {
  it("lists every page while the range stays short", () => {
    expect(buildPagerItems(3, 7)).toEqual([1, 2, 3, 4, 5, 6, 7]);
  });

  it("keeps the ends and the current neighbourhood, gapping the rest", () => {
    expect(buildPagerItems(5, 12)).toEqual([1, "gap", 4, 5, 6, "gap", 12]);
  });

  it("opens without a leading gap on the first pages", () => {
    expect(buildPagerItems(1, 12)).toEqual([1, 2, "gap", 12]);
  });

  it("closes without a trailing gap on the last pages", () => {
    expect(buildPagerItems(12, 12)).toEqual([1, "gap", 11, 12]);
  });

  it("never emits a gap that hides a single page", () => {
    expect(buildPagerItems(3, 8)).toEqual([1, 2, 3, 4, "gap", 8]);
  });
});
