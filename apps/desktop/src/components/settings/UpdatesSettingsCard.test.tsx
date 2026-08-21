import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const { getVersionMock } = vi.hoisted(() => ({ getVersionMock: vi.fn() }));
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: () => getVersionMock(),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: vi.fn(() => "macos"),
  version: vi.fn(() => "15.0"),
  arch: vi.fn(() => "aarch64"),
}));

const { updatePrefsMock, prefsState } = vi.hoisted(() => ({
  updatePrefsMock: vi.fn(),
  prefsState: { automaticUpdates: true },
}));
vi.mock("@/lib/desktop-prefs", () => ({
  useDesktopPrefs: () => ({
    prefs: { automaticUpdates: prefsState.automaticUpdates },
    updatePrefs: updatePrefsMock,
    setPrefs: vi.fn(),
  }),
}));

import { UpdatesSettingsCard } from "./UpdatesSettingsCard";
import { withQueryClient } from "@/test-utils/query-client";

function renderUpdatesSettings(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

function mockCommands(results: Partial<Record<string, unknown>>) {
  invokeMock.mockImplementation((command: string) =>
    command in results ? Promise.resolve(results[command]) : Promise.resolve(undefined),
  );
}

describe("UpdatesSettingsCard", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    getVersionMock.mockReset();
    updatePrefsMock.mockReset();
    prefsState.automaticUpdates = true;
    getVersionMock.mockResolvedValue("1.0.0");
    // Exercise release-only updater behavior.
    vi.stubEnv("DEV", false);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("shows the current app version", async () => {
    renderUpdatesSettings(<UpdatesSettingsCard />);
    expect(await screen.findByText("v1.0.0")).toBeInTheDocument();
  });

  it("toggles the automatic-updates preference", async () => {
    renderUpdatesSettings(<UpdatesSettingsCard />);
    fireEvent.click(screen.getByRole("button", { pressed: true }));
    expect(updatePrefsMock).toHaveBeenCalledWith({ automaticUpdates: false });
  });

  it("reports up-to-date after a manual check", async () => {
    mockCommands({ check_app_update: { kind: "up_to_date" } });
    renderUpdatesSettings(<UpdatesSettingsCard />);
    fireEvent.click(screen.getByRole("button", { name: /Check for updates/i }));
    expect(await screen.findByText(/on the latest version/i)).toBeInTheDocument();
  });

  it("offers an install action when an update is available", async () => {
    mockCommands({
      check_app_update: { kind: "available", version: "2.0.0", current_version: "1.0.0" },
    });
    renderUpdatesSettings(<UpdatesSettingsCard />);
    fireEvent.click(screen.getByRole("button", { name: /Check for updates/i }));
    expect(await screen.findByText(/2\.0\.0 is available/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Install and restart/i })).toBeInTheDocument();
  });

  it("refuses and surfaces a signature failure, and names the way out", async () => {
    mockCommands({ check_app_update: { kind: "signature_invalid", message: "bad sig" } });
    renderUpdatesSettings(<UpdatesSettingsCard />);
    fireEvent.click(screen.getByRole("button", { name: /Check for updates/i }));
    expect(await screen.findByText(/unverifiable signature/i)).toBeInTheDocument();
    // Checking again cannot clear this one, so the copy has to say what can.
    expect(screen.getByText(/sitecmd\.com/i)).toBeInTheDocument();
  });

  it("recovers when the update command rejects instead of resolving", async () => {
    invokeMock.mockImplementation((command: string) =>
      command === "check_app_update"
        ? Promise.reject(new Error("Updater does not have any endpoints set."))
        : Promise.resolve(undefined),
    );
    renderUpdatesSettings(<UpdatesSettingsCard />);
    fireEvent.click(screen.getByRole("button", { name: /Check for updates/i }));

    expect(
      await screen.findByText(/Something went wrong checking for updates/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Checking…/i)).toBeNull();
    // And the button is usable again, so the user can retry.
    expect(screen.getByRole("button", { name: /Check for updates/i })).toBeEnabled();
  });
});
