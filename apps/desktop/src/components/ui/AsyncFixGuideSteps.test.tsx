import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AsyncFixGuideSteps } from "./AsyncFixGuideSteps";

type Listener = (event: { payload: unknown }) => void;

const listeners = vi.hoisted(() => new Map<string, Set<Listener>>());

vi.mock("@/lib/tauri-events", () => ({
  safeListen: vi.fn(async (event: string, handler: Listener) => {
    let bucket = listeners.get(event);
    if (!bucket) {
      bucket = new Set();
      listeners.set(event, bucket);
    }
    bucket.add(handler);
    return () => bucket?.delete(handler);
  }),
}));

const { loadWebBaseline, loadWebFixGuide } = vi.hoisted(() => ({
  loadWebBaseline: vi.fn(),
  loadWebFixGuide: vi.fn(),
}));

vi.mock("@/lib/async-fix-guides", () => ({
  loadWebBaseline: (...args: unknown[]) => loadWebBaseline(...args),
  loadWebFixGuide: (...args: unknown[]) => loadWebFixGuide(...args),
  loadCodeBaseline: vi.fn(),
  loadCodeFixGuide: vi.fn(),
}));

// The rendering of a guide is FixGuideSteps' own contract; here only WHICH
// guide is on screen matters.
vi.mock("./FixGuideSteps", () => ({
  FixGuideSteps: ({ guide }: { guide: { title: string } }) => <div>{guide.title}</div>,
}));

function emitCatalogUpdated() {
  for (const handler of listeners.get("catalog-updated") ?? []) {
    handler({ payload: undefined });
  }
}

describe("AsyncFixGuideSteps", () => {
  beforeEach(() => {
    listeners.clear();
    loadWebBaseline.mockReset();
    loadWebFixGuide.mockReset();
  });

  it("reloads the open guide when a catalog pack activates, without flashing it away", async () => {
    let releaseSecondLoad: (guide: { title: string }) => void = () => {};
    loadWebFixGuide.mockResolvedValueOnce({ title: "baseline steps" }).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          releaseSecondLoad = resolve;
        }),
    );

    render(<AsyncFixGuideSteps kind="web" checkId="security.csp" />);
    await screen.findByText("baseline steps");
    await waitFor(() => expect(listeners.get("catalog-updated")?.size).toBeGreaterThan(0));

    emitCatalogUpdated();
    await waitFor(() => expect(loadWebFixGuide).toHaveBeenCalledTimes(2));

    expect(screen.getByText("baseline steps")).toBeTruthy();
    expect(screen.queryByText(/Loading fix guide/)).toBeNull();

    releaseSecondLoad({ title: "deep steps" });
    await screen.findByText("deep steps");
    expect(screen.queryByText("baseline steps")).toBeNull();
  });

  it("keeps baseline-only guides inert: no catalog subscription, no reload", async () => {
    loadWebBaseline.mockResolvedValue({ title: "baseline steps" });

    render(<AsyncFixGuideSteps kind="web" checkId="security.csp" baselineOnly />);
    await screen.findByText("baseline steps");

    // Bundled-only mode must not subscribe to catalog updates.
    expect(listeners.get("catalog-updated")?.size ?? 0).toBe(0);

    emitCatalogUpdated();
    expect(loadWebBaseline).toHaveBeenCalledTimes(1);
    expect(loadWebFixGuide).not.toHaveBeenCalled();
  });

  it("still clears the previous guide when the fix itself changes", async () => {
    loadWebFixGuide
      .mockResolvedValueOnce({ title: "csp steps" })
      .mockImplementationOnce(() => new Promise(() => {}));

    const { rerender } = render(<AsyncFixGuideSteps kind="web" checkId="security.csp" />);
    await screen.findByText("csp steps");

    rerender(<AsyncFixGuideSteps kind="web" checkId="seo.title" />);

    // A different fix cannot show stale steps while loading.
    await waitFor(() => expect(screen.queryByText("csp steps")).toBeNull());
    expect(screen.getByText(/Loading fix guide/)).toBeTruthy();
  });
});
