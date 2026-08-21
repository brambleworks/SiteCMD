import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { safeListen } from "@/lib/tauri-events";
import { useTauriEvent } from "./useTauriEvent";

type Listener = (event: { payload: unknown }) => void;

const listeners = new Map<string, Set<Listener>>();
const unlistenSpy = vi.fn();

vi.mock("@/lib/tauri-events", () => ({
  safeListen: vi.fn(async (event: string, handler: Listener) => {
    let bucket = listeners.get(event);
    if (!bucket) {
      bucket = new Set();
      listeners.set(event, bucket);
    }
    bucket.add(handler);
    return () => {
      unlistenSpy();
      bucket?.delete(handler);
    };
  }),
}));

const mockedSafeListen = vi.mocked(safeListen);

function emit(event: string, payload: unknown) {
  const bucket = listeners.get(event);
  if (!bucket) return;
  for (const handler of bucket) handler({ payload });
}

describe("useTauriEvent", () => {
  beforeEach(() => {
    listeners.clear();
    unlistenSpy.mockClear();
    mockedSafeListen.mockClear();
  });

  afterEach(() => {
    listeners.clear();
  });

  it("delivers the typed payload to the handler", async () => {
    const handler = vi.fn();
    renderHook(() => useTauriEvent("google-integration-updated", handler));

    await waitFor(() =>
      expect(listeners.get("google-integration-updated")?.size).toBeGreaterThan(0),
    );
    emit("google-integration-updated", { projectId: 7 });

    expect(handler).toHaveBeenCalledWith({ projectId: 7 });
  });

  it("calls the latest handler without re-subscribing when the closure changes", async () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = renderHook(
      ({ handler }) => useTauriEvent("fix-attempt-updated", handler),
      {
        initialProps: { handler: first },
      },
    );

    await waitFor(() => expect(listeners.get("fix-attempt-updated")?.size).toBeGreaterThan(0));
    expect(mockedSafeListen).toHaveBeenCalledTimes(1);

    rerender({ handler: second });
    expect(mockedSafeListen).toHaveBeenCalledTimes(1);

    emit("fix-attempt-updated", undefined);
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
  });

  it("does not subscribe while disabled and attaches once enabled", async () => {
    const handler = vi.fn();
    const { rerender } = renderHook(
      ({ enabled }) => useTauriEvent("fix-attempt-updated", handler, { enabled }),
      { initialProps: { enabled: false } },
    );

    expect(mockedSafeListen).not.toHaveBeenCalled();
    expect(listeners.get("fix-attempt-updated")?.size ?? 0).toBe(0);

    rerender({ enabled: true });
    await waitFor(() => expect(listeners.get("fix-attempt-updated")?.size).toBeGreaterThan(0));

    emit("fix-attempt-updated", undefined);
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("detaches the listener on unmount", async () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() => useTauriEvent("fix-attempt-updated", handler));

    await waitFor(() => expect(listeners.get("fix-attempt-updated")?.size).toBeGreaterThan(0));
    unmount();

    await waitFor(() => expect(unlistenSpy).toHaveBeenCalled());
    expect(listeners.get("fix-attempt-updated")?.size ?? 0).toBe(0);
  });

  it("unlistens a late-attaching listener when unmounted before safeListen resolves", async () => {
    let resolveListen: (stop: () => void) => void = () => {};
    const stop = vi.fn();
    mockedSafeListen.mockImplementationOnce(
      () =>
        new Promise<() => void>((resolve) => {
          resolveListen = resolve;
        }),
    );

    const { unmount } = renderHook(() => useTauriEvent("fix-attempt-updated", vi.fn()));
    unmount();

    // The listener resolves only after unmount.
    resolveListen(stop);
    await waitFor(() => expect(stop).toHaveBeenCalledTimes(1));
  });
});
