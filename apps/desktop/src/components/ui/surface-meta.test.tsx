import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MS_PER_MINUTE } from "@/lib/format";
import { FreshnessBadge } from "./surface-meta";

afterEach(() => {
  vi.useRealTimers();
});

describe("FreshnessBadge", () => {
  it("becomes stale as time passes without a parent render", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-20T12:00:00Z"));

    render(
      <FreshnessBadge
        timestamp={new Date("2026-08-20T11:31:00Z")}
        prefix="Updated"
        staleAfterMs={30 * MS_PER_MINUTE}
      />,
    );

    expect(screen.getByText("Updated 29m ago")).toHaveClass("text-emerald-300");

    act(() => {
      vi.advanceTimersByTime(2 * MS_PER_MINUTE);
    });

    expect(screen.getByText("Updated 31m ago")).toHaveClass("text-amber-300");
  });
});
