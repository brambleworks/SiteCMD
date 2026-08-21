import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SslProbeResult } from "@/lib/dashboard/types";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: invokeMock,
}));

import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { invalidateDashboardSslProbe, useDashboardSslProbe } from "./useDashboardSslProbe";

const PROBE_RESULT: SslProbeResult = {
  days_remaining: 34,
  auto_renew_hint: true,
  not_after_iso: "2026-06-22T12:00:00Z",
  error: null,
};

function useArmedProbe(url: string) {
  return useDashboardSslProbe({
    auxiliarySignalsArmed: true,
    includeReferenceSignals: true,
    url,
  });
}

describe("useDashboardSslProbe", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    window.sessionStorage.clear();
  });

  it("reuses the in-session cache before auxiliary signals are armed", async () => {
    const url = "https://ssl-cache.example";
    invokeMock.mockResolvedValue(PROBE_RESULT);
    const wrapper = withQueryClient(createTestQueryClient());

    const first = renderHook(() => useArmedProbe(url), { wrapper });
    await waitFor(() => expect(first.result.current?.days_remaining).toBe(34));
    expect(invokeMock).toHaveBeenCalledTimes(1);
    first.unmount();

    invokeMock.mockClear();
    const second = renderHook(
      () =>
        useDashboardSslProbe({ auxiliarySignalsArmed: false, includeReferenceSignals: true, url }),
      { wrapper },
    );

    expect(second.result.current?.days_remaining).toBe(34);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("hydrates a fresh client from the persisted probe without re-probing", async () => {
    const url = "https://ssl-reload.example";
    invokeMock.mockResolvedValue(PROBE_RESULT);

    // First client probes and writes the sessionStorage tier.
    const first = renderHook(() => useArmedProbe(url), {
      wrapper: withQueryClient(createTestQueryClient()),
    });
    await waitFor(() => expect(first.result.current?.days_remaining).toBe(34));
    first.unmount();
    invokeMock.mockClear();

    // A fresh client (a reload) has an empty in-memory cache; the persisted,
    // still-fresh entry seeds it instantly so no second probe is issued.
    const reloaded = renderHook(() => useArmedProbe(url), {
      wrapper: withQueryClient(createTestQueryClient()),
    });

    expect(reloaded.result.current?.days_remaining).toBe(34);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-probes after explicit invalidation", async () => {
    const url = "https://ssl-refresh.example";
    invokeMock.mockResolvedValue(PROBE_RESULT);
    const client = createTestQueryClient();
    const wrapper = withQueryClient(client);

    const first = renderHook(() => useArmedProbe(url), { wrapper });
    await waitFor(() => expect(first.result.current?.days_remaining).toBe(34));
    first.unmount();

    invalidateDashboardSslProbe(client, url);
    invokeMock.mockResolvedValue({ ...PROBE_RESULT, days_remaining: 33 });

    const second = renderHook(() => useArmedProbe(url), { wrapper });

    await waitFor(() => expect(second.result.current?.days_remaining).toBe(33));
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });
});
