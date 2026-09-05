import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { queryKeys } from "@/lib/query/query-keys";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { useEvents } from "./useEvents";

describe("useEvents", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("parses event detail during full loads", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "backfill_events") return 0;
      if (command === "get_events") {
        return [
          {
            id: 1,
            projectId: 7,
            eventType: "scan",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-12T10:00:00Z"),
            title: "SiteCMD Score: 81/100",
            summary: "1 issue",
            detail: '{"score":81}',
            source: "internal",
            sourceId: "scan-1",
          },
        ];
      }
      return null;
    });

    const { result } = renderHook(() => useEvents(7), { wrapper: withQueryClient() });

    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });

    expect(result.current.events).toHaveLength(1);
    expect(result.current.events[0]?.parsedDetail).toEqual({ score: 81 });
  });

  it("does not block the first load on backfill work", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "backfill_events") {
        return new Promise(() => {});
      }
      if (command === "get_events") {
        return Promise.resolve([
          {
            id: 1,
            projectId: 7,
            eventType: "scan",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-12T10:00:00Z"),
            title: "SiteCMD Score: 81/100",
            summary: "1 issue",
            detail: '{"score":81}',
            source: "internal",
            sourceId: "scan-1",
          },
        ]);
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useEvents(7), { wrapper: withQueryClient() });

    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });

    expect(result.current.events).toHaveLength(1);
  });

  it("reloads only newer events on focus and merges same-timestamp events by id", async () => {
    const firstEvent = {
      id: 1,
      projectId: 7,
      eventType: "scan",
      severity: "info",
      occurredAtMs: Date.parse("2026-04-12T10:00:00Z"),
      title: "SiteCMD Score: 81/100",
      summary: "1 issue",
      detail: '{"score":81}',
      source: "internal",
      sourceId: "scan-1",
    };
    const secondEvent = {
      id: 2,
      projectId: 7,
      eventType: "verification",
      severity: "warning",
      occurredAtMs: Date.parse("2026-04-12T10:00:00Z"),
      title: "Today verify sweep: 1 issue still open",
      summary: "Re-checked 1 item.",
      detail: '{"rechecked_count":1}',
      source: "internal",
      sourceId: "verify-1",
    };

    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "backfill_events") return 0;
      if (command === "get_events") {
        if (args?.sinceMs != null) {
          expect(args.sinceMs).toBe(firstEvent.occurredAtMs);
          expect(args.sinceEventId).toBe(firstEvent.id);
          expect(args.limit).toBe(500);
          return [secondEvent];
        }
        expect(args?.limit).toBe(501);
        return [firstEvent];
      }
      return null;
    });

    const { result } = renderHook(() => useEvents(7), { wrapper: withQueryClient() });

    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });

    act(() => {
      window.dispatchEvent(new Event("focus"));
    });

    await waitFor(() => {
      expect(result.current.events.map((event) => event.id)).toEqual([2, 1]);
    });

    expect(result.current.events[0]?.parsedDetail).toEqual({ rechecked_count: 1 });
  });

  it("only backfills each project once per hook session", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "backfill_events") return 0;
      if (command === "get_events") return [];
      return null;
    });

    const { result, rerender } = renderHook(
      ({ projectId }: { projectId: number | null }) => useEvents(projectId),
      { initialProps: { projectId: 7 }, wrapper: withQueryClient() },
    );

    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });

    rerender({ projectId: 8 });
    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });

    rerender({ projectId: 7 });
    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });

    const backfillCalls = invokeMock.mock.calls.filter(
      ([command]) => command === "backfill_events",
    );
    expect(backfillCalls).toHaveLength(2);
    expect(backfillCalls.map(([, args]) => (args as { projectId: number }).projectId)).toEqual([
      7, 8,
    ]);
  });

  it("keeps only the newest 500 events and reports overflow after incremental polling", async () => {
    const batch = (endId: number) =>
      Array.from({ length: 500 }, (_, index) => ({
        id: endId - index,
        projectId: 7,
        eventType: "scan",
        severity: "info",
        occurredAtMs: Date.parse("2026-04-12T10:00:00Z") + endId - index,
        title: `Event ${endId - index}`,
        summary: "",
        detail: null,
        source: "internal",
        sourceId: `scan-${endId - index}`,
      }));
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "backfill_events") return new Promise(() => {});
      if (command === "get_events")
        return Promise.resolve(batch(Number(args?.sinceEventId ?? 0) + 500));
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useEvents(7), { wrapper: withQueryClient() });
    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });
    expect(result.current.events).toHaveLength(500);
    expect(result.current.hasMore).toBe(false);

    for (const newestId of [1000, 1500, 2000]) {
      act(() => window.dispatchEvent(new Event("focus")));
      await waitFor(() => expect(result.current.events[0]?.id).toBe(newestId));
      expect(result.current.events).toHaveLength(500);
      expect(result.current.events.at(-1)?.id).toBe(newestId - 499);
      expect(result.current.hasMore).toBe(true);
    }
  });

  it("exposes a real error when the initial event load fails", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "backfill_events") return 0;
      if (command === "get_events") {
        throw new Error("offline");
      }
      return null;
    });

    const { result } = renderHook(() => useEvents(7), { wrapper: withQueryClient() });

    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });

    await waitFor(() => {
      expect(result.current.error).toBe("Activity could not load right now.");
    });
    expect(result.current.events).toEqual([]);
  });

  it("stays out of the loading state during a background refetch", async () => {
    const client = createTestQueryClient();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "backfill_events") return 0;
      if (command === "get_events") return [];
      return null;
    });

    const { result } = renderHook(() => useEvents(7), { wrapper: withQueryClient(client) });

    await act(async () => {
      await result.current.loadEvents("2026-04-01T00:00:00Z", "2026-04-30T23:59:59Z");
    });
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await client.invalidateQueries({ queryKey: queryKeys.events.all });
    });

    expect(result.current.loading).toBe(false);
  });
});
