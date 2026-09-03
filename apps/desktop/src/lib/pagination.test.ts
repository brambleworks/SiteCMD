import { describe, expect, it } from "vitest";
import { buildPagerItems, pageWindow } from "@/lib/pagination";

describe("pageWindow", () => {
  const rows = Array.from({ length: 3000 }, (_, index) => index + 1);

  it("bounds a huge list to one page of rows", () => {
    const bounded = pageWindow(rows, 1, 50);
    expect(bounded.rows).toHaveLength(50);
    expect(bounded.rows[0]).toBe(1);
    expect(bounded.totalPages).toBe(60);
  });

  it("moves the window with the requested page", () => {
    expect(pageWindow(rows, 3, 50).rows[0]).toBe(101);
  });

  it("returns a short final page rather than padding it", () => {
    const bounded = pageWindow(rows.slice(0, 120), 3, 50);
    expect(bounded.rows).toHaveLength(20);
    expect(bounded.rows[0]).toBe(101);
  });

  it("clamps a page beyond the end back onto the last page", () => {
    const bounded = pageWindow(rows.slice(0, 10), 9, 50);
    expect(bounded.page).toBe(1);
    expect(bounded.rows).toHaveLength(10);
  });

  it("keeps an empty list on a single page", () => {
    expect(pageWindow([], 1, 50)).toEqual({ page: 1, totalPages: 1, rows: [] });
  });
});

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
