import { beforeEach, describe, expect, it, vi } from "vitest";
import { migrateFromLocalStorage, storeGet, storeSet } from "./store";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));

function parsePreference(value: unknown): { enabled: boolean } | null {
  if (typeof value !== "object" || value === null || !("enabled" in value)) return null;
  return typeof value.enabled === "boolean" ? { enabled: value.enabled } : null;
}

describe("fixed application settings store", () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue(null);
    window.localStorage.clear();
  });

  it("reads through the settings command and preserves false values", async () => {
    invokeMock.mockResolvedValue(false);

    await expect(storeGet("preference", true)).resolves.toBe(false);
    expect(invokeMock).toHaveBeenCalledWith("get_app_setting", { key: "preference" });
  });

  it("uses the fallback for missing values or an unavailable backend", async () => {
    await expect(storeGet("missing", 7)).resolves.toBe(7);
    invokeMock.mockRejectedValue(new Error("store unavailable"));
    await expect(storeGet("missing", 7)).resolves.toBe(7);
  });

  it("passes path-shaped keys only as setting keys", async () => {
    const key = "../../outside.json";
    await storeSet(key, { enabled: true });

    expect(invokeMock).toHaveBeenCalledWith("set_app_setting", {
      key,
      value: { enabled: true },
    });
  });

  it("keeps the localStorage fallback usable when persistence fails", async () => {
    invokeMock.mockRejectedValue(new Error("store unavailable"));
    await expect(storeSet("preference", true)).resolves.toBeUndefined();
  });

  it("preserves existing durable preferences when migrating localStorage", async () => {
    invokeMock.mockResolvedValue({ enabled: false });
    window.localStorage.setItem("old-preference", JSON.stringify({ enabled: true }));

    await expect(
      migrateFromLocalStorage("old-preference", "preference", { enabled: true }, parsePreference),
    ).resolves.toEqual({ enabled: false });
    expect(JSON.parse(window.localStorage.getItem("old-preference")!)).toEqual({ enabled: false });
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("migrates valid local preferences through the restricted write command", async () => {
    window.localStorage.setItem("old-preference", JSON.stringify({ enabled: true }));

    await expect(
      migrateFromLocalStorage("old-preference", "preference", { enabled: false }, parsePreference),
    ).resolves.toEqual({ enabled: true });
    expect(invokeMock).toHaveBeenLastCalledWith("set_app_setting", {
      key: "preference",
      value: { enabled: true },
    });
  });

  it("does not persist malformed local preferences", async () => {
    window.localStorage.setItem("old-preference", JSON.stringify({ enabled: "invalid" }));

    await expect(
      migrateFromLocalStorage("old-preference", "preference", { enabled: false }, parsePreference),
    ).resolves.toEqual({ enabled: false });
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});
