import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { SUPPORT_EMAIL } from "@/lib/support";

// Captured handlers let tests deliver late bridge outcomes.
const lateState = vi.hoisted(() => ({
  handlers: new Set<(late: unknown) => void>(),
}));
vi.mock("@/lib/privileged-command-bridge", () => ({
  onLatePrivilegedResolution: (handler: (late: unknown) => void) => {
    lateState.handlers.add(handler);
    return () => lateState.handlers.delete(handler);
  },
}));

function deliverLate(late: unknown) {
  act(() => {
    for (const handler of [...lateState.handlers]) handler(late);
  });
}

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const { openUrlMock } = vi.hoisted(() => ({
  openUrlMock: vi.fn(),
}));

vi.mock("@/lib/open-url", () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...args),
}));

const prefsState = vi.hoisted(() => ({ automaticUpdates: false }));
vi.mock("@/lib/desktop-prefs", () => ({
  useDesktopPrefs: () => ({
    prefs: { automaticUpdates: prefsState.automaticUpdates },
    updatePrefs: vi.fn(),
    setPrefs: vi.fn(),
  }),
}));

import { UpdateBanner } from "./UpdateBanner";

type CommandResults = Partial<Record<string, unknown>>;

function mockCommands(results: CommandResults) {
  invokeMock.mockImplementation((command: string) => {
    if (command in results) return Promise.resolve(results[command]);
    return Promise.resolve(undefined);
  });
}

describe("UpdateBanner", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openUrlMock.mockReset();
    localStorage.clear();
    sessionStorage.clear();
    prefsState.automaticUpdates = false;
    lateState.handlers.clear();
  });

  it("keeps a previously dismissed update hidden when only the legacy key exists", async () => {
    localStorage.setItem("shk-dismissed-update", "2.0.0");
    mockCommands({
      check_app_update: { kind: "available", version: "2.0.0", current_version: "1.9.0" },
    });

    render(<UpdateBanner />);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("check_app_update");
    });

    expect(screen.queryByText("Download")).toBeNull();
  });

  it("manual mode: links to the download page, offers in-app install, writes the dismissal key", async () => {
    mockCommands({
      check_app_update: { kind: "available", version: "2.0.0", current_version: "1.9.0" },
    });

    render(<UpdateBanner />);

    const download = await screen.findByRole("link", { name: "Download" });
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeInTheDocument();
    fireEvent.click(download);
    expect(openUrlMock).toHaveBeenCalledWith("https://sitecmd.com/download");

    fireEvent.click(screen.getByTitle("Dismiss"));
    expect(localStorage.getItem("sitecmd-dismissed-update")).toBe("2.0.0");
    expect(localStorage.getItem("shk-dismissed-update")).toBeNull();
  });

  it("manual mode: clicking Install installs then relaunches", async () => {
    mockCommands({
      check_app_update: { kind: "available", version: "2.0.0", current_version: "1.9.0" },
      download_and_install_app_update: { kind: "installed", version: "2.0.0" },
    });

    render(<UpdateBanner />);

    fireEvent.click(await screen.findByRole("button", { name: "Install and restart" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("download_and_install_app_update");
      expect(invokeMock).toHaveBeenCalledWith("restart_app");
    });
  });

  it("auto mode: installs silently then shows a restart-to-apply pill", async () => {
    prefsState.automaticUpdates = true;
    mockCommands({
      check_app_update: { kind: "available", version: "2.0.0", current_version: "1.9.0" },
      download_and_install_app_update: { kind: "installed", version: "2.0.0" },
    });

    render(<UpdateBanner />);

    // No manual "Install and restart" / "Download" banner in auto mode.
    expect(screen.queryByRole("button", { name: "Install and restart" })).toBeNull();
    // The silent install runs, then the restart pill appears; it has NOT
    // auto-relaunched (restart waits for the user's nod).
    expect(await screen.findByText(/Restart to finish/i)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("download_and_install_app_update");
    expect(invokeMock).not.toHaveBeenCalledWith("restart_app");

    fireEvent.click(screen.getByRole("button", { name: "Restart now" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("restart_app"));
  });

  it("auto mode: a failed silent install falls back to the manual banner", async () => {
    prefsState.automaticUpdates = true;
    mockCommands({
      check_app_update: { kind: "available", version: "2.0.0", current_version: "1.9.0" },
      download_and_install_app_update: { kind: "network_unavailable", message: "dns" },
    });

    render(<UpdateBanner />);

    expect(await screen.findByText(/download it manually/i)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Download" })).toBeInTheDocument();
  });

  it("auto mode: an install REJECTION falls back instead of stranding the silent phase", async () => {
    prefsState.automaticUpdates = true;
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_app_update") {
        return Promise.resolve({ kind: "available", version: "2.0.0", current_version: "1.9.0" });
      }
      if (command === "download_and_install_app_update") {
        return Promise.reject(new Error("That action took too long to finish."));
      }
      return Promise.resolve(undefined);
    });

    render(<UpdateBanner />);

    expect(await screen.findByText(/download it manually/i)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Download" })).toBeInTheDocument();
  });

  it("a TIMEOUT rejection says the install may still finish, not that it failed", async () => {
    // A bridge timeout does not prove that the native installer stopped.
    prefsState.automaticUpdates = true;
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_app_update") {
        return Promise.resolve({ kind: "available", version: "2.0.0", current_version: "1.9.0" });
      }
      if (command === "download_and_install_app_update") {
        return Promise.reject(
          Object.assign(new Error("That action took too long to finish."), {
            command: "download_and_install_app_update",
            timeoutMs: 600000,
          }),
        );
      }
      return Promise.resolve(undefined);
    });

    render(<UpdateBanner />);

    expect(await screen.findByText(/may still finish in the background/i)).toBeInTheDocument();
    // Hide retry while the native install may still be running.
    expect(screen.queryByRole("button", { name: "Install and restart" })).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Download" })).toBeInTheDocument();
  });

  it("a late success after the timeout replaces the reassurance with the ready pill", async () => {
    // A late native outcome replaces the timeout state.
    prefsState.automaticUpdates = true;
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_app_update") {
        return Promise.resolve({ kind: "available", version: "2.0.0", current_version: "1.9.0" });
      }
      if (command === "download_and_install_app_update") {
        return Promise.reject(
          Object.assign(new Error("That action took too long to finish."), {
            command: "download_and_install_app_update",
            timeoutMs: 600000,
          }),
        );
      }
      return Promise.resolve(undefined);
    });

    render(<UpdateBanner />);
    // This timeout guards hangs without imposing a performance budget under load.
    const settle = { timeout: 10_000 };
    expect(
      await screen.findByText(/may still finish in the background/i, undefined, settle),
    ).toBeInTheDocument();

    deliverLate({
      command: "download_and_install_app_update",
      ok: true,
      value: { kind: "installed", version: "2.0.0" },
    });

    expect(await screen.findByText("Update installed.", undefined, settle)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restart now" })).toBeInTheDocument();
  }, 20_000);

  it("a late signature refusal after the timeout surfaces the refusal banner", async () => {
    prefsState.automaticUpdates = true;
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_app_update") {
        return Promise.resolve({ kind: "available", version: "2.0.0", current_version: "1.9.0" });
      }
      if (command === "download_and_install_app_update") {
        return Promise.reject(
          Object.assign(new Error("That action took too long to finish."), {
            command: "download_and_install_app_update",
            timeoutMs: 600000,
          }),
        );
      }
      return Promise.resolve(undefined);
    });

    render(<UpdateBanner />);
    expect(await screen.findByText(/may still finish in the background/i)).toBeInTheDocument();

    deliverLate({
      command: "download_and_install_app_update",
      ok: true,
      value: { kind: "signature_invalid", message: "minisign verification failed" },
    });

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/update refused/i);
    expect(screen.queryByText(/may still finish in the background/i)).not.toBeInTheDocument();
  });

  it("a rejected relaunch after a successful manual install lands on the ready pill", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_app_update") {
        return Promise.resolve({ kind: "available", version: "2.0.0", current_version: "1.9.0" });
      }
      if (command === "download_and_install_app_update") {
        return Promise.resolve({ kind: "installed" });
      }
      if (command === "restart_app") {
        return Promise.reject(new Error("restart IPC failed"));
      }
      return Promise.resolve(undefined);
    });

    render(<UpdateBanner />);
    fireEvent.click(await screen.findByRole("button", { name: "Install and restart" }));

    expect(await screen.findByText("Update installed.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restart now" })).toBeInTheDocument();
  });

  it("renders nothing for up_to_date / network_unavailable / unknown outcomes (P3.1)", async () => {
    mockCommands({ check_app_update: { kind: "up_to_date" } });
    const { container, rerender } = render(<UpdateBanner />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();

    invokeMock.mockReset();
    mockCommands({
      check_app_update: { kind: "network_unavailable", message: "dns lookup failed" },
    });
    rerender(<UpdateBanner key="net" />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();

    invokeMock.mockReset();
    mockCommands({ check_app_update: { kind: "unknown", message: "novel updater error" } });
    rerender(<UpdateBanner key="unk" />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("surfaces a loud refusal banner for signature_invalid outcomes (P3.1)", async () => {
    mockCommands({
      check_app_update: { kind: "signature_invalid", message: "signature verification failed" },
    });

    render(<UpdateBanner />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/Update refused/i);
    expect(alert).toHaveTextContent(/unverifiable signature/i);
    expect(alert).toHaveTextContent(SUPPORT_EMAIL);
  });

  it("offers the download page on a signature refusal, because retrying cannot fix one", async () => {
    mockCommands({
      check_app_update: { kind: "signature_invalid", message: "signature verification failed" },
    });

    render(<UpdateBanner />);

    const download = await screen.findByRole("link", { name: "Download" });
    fireEvent.click(download);
    expect(openUrlMock).toHaveBeenCalledWith("https://sitecmd.com/download");
  });

  it("session-dismissing a signature-invalid banner keeps it hidden until next launch", async () => {
    mockCommands({
      check_app_update: { kind: "signature_invalid", message: "signature verification failed" },
    });

    const { rerender } = render(<UpdateBanner />);
    fireEvent.click(await screen.findByTitle(/Dismiss for this session/i));
    expect(sessionStorage.getItem("sitecmd-dismissed-update-signature")).toBe("1");

    rerender(<UpdateBanner key="redraw" />);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
