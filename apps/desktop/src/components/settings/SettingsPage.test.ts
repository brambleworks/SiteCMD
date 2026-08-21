import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri-invoke", () => ({ invoke: vi.fn(() => Promise.resolve(null)) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import { normalizeTab } from "./SettingsPage";

describe("normalizeTab", () => {
  it("passes through current tab names", () => {
    expect(normalizeTab("site-setup")).toBe("site-setup");
    expect(normalizeTab("scanning")).toBe("scanning");
    expect(normalizeTab("automation")).toBe("automation");
    expect(normalizeTab("connected")).toBe("connected");
    expect(normalizeTab("account")).toBe("account");
    expect(normalizeTab("app-preferences")).toBe("app-preferences");
    expect(normalizeTab("privacy-diagnostics")).toBe("privacy-diagnostics");
    expect(normalizeTab("data")).toBe("data");
  });

  it("collapses deprecated tab names to their new settings sections", () => {
    expect(normalizeTab("project")).toBe("site-setup");
    expect(normalizeTab("project-basics")).toBe("site-setup");
    expect(normalizeTab("pages")).toBe("site-setup");
    expect(normalizeTab("danger-zone")).toBe("site-setup");
    expect(normalizeTab("danger")).toBe("site-setup");
    expect(normalizeTab("scan-settings")).toBe("scanning");
    expect(normalizeTab("scan-defaults")).toBe("scanning");
    expect(normalizeTab("scan-prefs")).toBe("scanning");
    expect(normalizeTab("scan-behavior")).toBe("scanning");
    expect(normalizeTab("schedules")).toBe("scanning");
    expect(normalizeTab("scheduled-scans")).toBe("scanning");
    expect(normalizeTab("automations")).toBe("automation");
    expect(normalizeTab("cicd")).toBe("automation");
    expect(normalizeTab("ci")).toBe("automation");
    expect(normalizeTab("ci-cd")).toBe("automation");
    expect(normalizeTab("webhooks")).toBe("automation");
    expect(normalizeTab("webhook")).toBe("automation");
    expect(normalizeTab("billing")).toBe("account");
    expect(normalizeTab("general")).toBe("app-preferences");
    expect(normalizeTab("appearance")).toBe("app-preferences");
    expect(normalizeTab("about")).toBe("app-preferences");
    expect(normalizeTab("telemetry")).toBe("privacy-diagnostics");
    expect(normalizeTab("privacy")).toBe("privacy-diagnostics");
    expect(normalizeTab("diagnostics")).toBe("privacy-diagnostics");
    expect(normalizeTab("data-support")).toBe("data");
    expect(normalizeTab("support")).toBe("data");
    expect(normalizeTab("backups")).toBe("data");
  });

  it("no longer claims the reports page; that deep link goes to the real Reports page", () => {
    expect(normalizeTab("reports")).toBe("account");
  });

  it("defaults unknown tabs to 'account'", () => {
    expect(normalizeTab("bogus")).toBe("account");
    expect(normalizeTab("")).toBe("account");
    expect(normalizeTab(undefined)).toBe("account");
  });
});
