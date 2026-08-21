import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeCalls: { command: string; args?: unknown }[] = [];
const invokeMock = vi.fn();

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (command: string, args?: unknown) => {
    invokeCalls.push({ command, args });
    return invokeMock(command, args);
  },
}));

// Captured handlers let tests deliver late bridge verdicts.
const lateState = vi.hoisted(() => ({
  handlers: new Set<(late: unknown) => void>(),
}));
vi.mock("@/lib/privileged-command-bridge", () => ({
  onLatePrivilegedResolution: (handler: (late: unknown) => void) => {
    lateState.handlers.add(handler);
    return () => lateState.handlers.delete(handler);
  },
}));

const { TierProvider, useTier } = await import("./useTier");

function wrapper({ children }: { children: ReactNode }) {
  return <TierProvider>{children}</TierProvider>;
}

const FREE_LICENSE = {
  tier: "free" as const,
  status: "none",
  planName: "Free",
  billingInterval: null,
  isActive: false,
  expiresAt: null,
  features: [],
  checkoutUrls: {
    core: "",
    pro: "",
    coreMonthly: "",
    coreAnnual: "",
    proMonthly: "",
    proAnnual: "",
  },
  customerPortalUrl: "",
  validationWarning: "none" as const,
};

describe("TierProvider", () => {
  beforeEach(() => {
    invokeCalls.length = 0;
    invokeMock.mockReset();
    lateState.handlers.clear();
  });

  afterEach(() => {
    invokeMock.mockReset();
  });

  it("loads license state without calling legacy custom trial commands", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_license_status") return Promise.resolve(FREE_LICENSE);
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTier(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.tier).toBe("free");
    expect(invokeCalls.map((call) => call.command)).toEqual(["get_license_status"]);
    expect(invokeCalls.some((call) => call.command.includes("trial"))).toBe(false);
  });

  it("re-attaches a validation verdict that arrived after the bridge budget", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_license_status") return Promise.resolve(FREE_LICENSE);
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTier(), { wrapper });
    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });
    expect(result.current.tier).toBe("free");

    act(() => {
      for (const handler of [...lateState.handlers]) {
        handler({
          command: "validate_license",
          ok: true,
          value: {
            ...FREE_LICENSE,
            tier: "pro",
            planName: "Pro",
            isActive: true,
            status: "active",
          },
        });
      }
    });

    await waitFor(() => {
      expect(result.current.tier).toBe("pro");
    });
  });

  it("auto-revalidates when get_license_status returns a stale warning", async () => {
    const staleLicense = {
      ...FREE_LICENSE,
      tier: "core" as const,
      validationWarning: "stale" as const,
    };
    const refreshedLicense = {
      ...staleLicense,
      validationWarning: "none" as const,
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_license_status") return Promise.resolve(staleLicense);
      if (command === "validate_license") return Promise.resolve(refreshedLicense);
      return Promise.resolve(null);
    });

    renderHook(() => useTier(), { wrapper });

    await waitFor(() => {
      expect(invokeCalls.filter((call) => call.command === "validate_license")).toHaveLength(1);
    });
    // Unforced: the background path exists to keep the cache warm, and the
    // backend's own >24h gate decides whether a live round trip happens.
    const call = invokeCalls.find((c) => c.command === "validate_license");
    expect(call?.args).toBeUndefined();
  });

  it("passes force through when the caller asks for a live check", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_license_status") return Promise.resolve(FREE_LICENSE);
      if (command === "validate_license") return Promise.resolve(FREE_LICENSE);
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTier(), { wrapper });
    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await result.current.refreshLicense({ force: true });

    const call = invokeCalls.find((c) => c.command === "validate_license");
    expect(call?.args).toEqual({ force: true });
  });

  it("leaves a bare refresh unforced so recovery re-reads stay cache-priced", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_license_status") return Promise.resolve(FREE_LICENSE);
      if (command === "validate_license") return Promise.resolve(FREE_LICENSE);
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTier(), { wrapper });
    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await result.current.refreshLicense();

    const call = invokeCalls.find((c) => c.command === "validate_license");
    expect(call?.args).toBeUndefined();
  });

  it("does not auto-revalidate when get_license_status returns a fresh license", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_license_status") return Promise.resolve(FREE_LICENSE);
      return Promise.resolve(null);
    });

    renderHook(() => useTier(), { wrapper });

    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(invokeCalls.filter((call) => call.command === "validate_license")).toHaveLength(0);
  });
});
