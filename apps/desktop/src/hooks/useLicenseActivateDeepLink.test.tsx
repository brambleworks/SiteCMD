import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useLicenseActivateDeepLink } from "./useLicenseActivateDeepLink";

const LICENSE_ACTIVATE_EVENT = "sitecmd-license-activate-requested";

type Listener = (event: { payload: unknown }) => void;

const listeners = new Map<string, Set<Listener>>();
const activateLicense = vi.fn();
const refreshLicense = vi.fn().mockResolvedValue({ isActive: false, tier: "free" });
const toastSuccess = vi.fn();
const toastError = vi.fn();
const toastWarning = vi.fn();
const toastInfo = vi.fn();

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

// Cold-start links arrive through the plugin rather than the runtime event.
const getCurrentDeepLinks = vi.fn();

vi.mock("@tauri-apps/plugin-deep-link", () => ({
  getCurrent: (...args: unknown[]) => getCurrentDeepLinks(...args),
}));

const confirmLinkLicenseActivation = vi.fn();

vi.mock("@/lib/commands", () => ({
  confirmLinkLicenseActivation: (...args: unknown[]) => confirmLinkLicenseActivation(...args),
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({ activateLicense, refreshLicense }),
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: toastSuccess,
    error: toastError,
    warning: toastWarning,
    info: toastInfo,
  }),
}));

function emitDeepLink(payload: unknown) {
  const bucket = listeners.get(LICENSE_ACTIVATE_EVENT);
  if (!bucket) return;
  for (const handler of bucket) handler({ payload });
}

describe("useLicenseActivateDeepLink", () => {
  beforeEach(() => {
    listeners.clear();
    activateLicense.mockReset();
    refreshLicense.mockReset();
    refreshLicense.mockResolvedValue({ isActive: false, tier: "free" });
    toastSuccess.mockReset();
    toastError.mockReset();
    toastWarning.mockReset();
    toastInfo.mockReset();
    getCurrentDeepLinks.mockReset();
    getCurrentDeepLinks.mockResolvedValue(null);
    confirmLinkLicenseActivation.mockReset();
    confirmLinkLicenseActivation.mockResolvedValue(true);
  });

  afterEach(() => {
    listeners.clear();
  });

  it("asks before installing a key that arrived over a link", async () => {
    activateLicense.mockResolvedValue({ tier: "core", isActive: true });

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-key-001" }); // gitleaks:allow

    await waitFor(() => expect(activateLicense).toHaveBeenCalled());
    expect(confirmLinkLicenseActivation).toHaveBeenCalledOnce();
  });

  it("installs nothing when the activation is declined", async () => {
    confirmLinkLicenseActivation.mockResolvedValue(false);

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-key-001" }); // gitleaks:allow

    await waitFor(() => expect(confirmLinkLicenseActivation).toHaveBeenCalledOnce());
    expect(activateLicense).not.toHaveBeenCalled();
    // Declining is a choice, not a failure, so it draws no error or warning.
    expect(toastError).not.toHaveBeenCalled();
    expect(toastWarning).not.toHaveBeenCalled();
  });

  it("acknowledges a refusal instead of falling silent", async () => {
    confirmLinkLicenseActivation.mockResolvedValue(false);

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-key-001" }); // gitleaks:allow

    await waitFor(() => expect(toastInfo).toHaveBeenCalled());
    const [, body] = toastInfo.mock.calls.at(-1) ?? [];
    expect(body).toContain("Nothing was changed");
    expect(body).toContain("Settings");
    expect(activateLicense).not.toHaveBeenCalled();
  });

  it("says so when the confirmation could not be asked at all", async () => {
    confirmLinkLicenseActivation.mockRejectedValue(new Error("not allowed by scope"));

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-key-001" }); // gitleaks:allow

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    const [, body] = toastError.mock.calls.at(-1) ?? [];
    expect(body).toContain("not allowed by scope");
    expect(body).toContain("Settings");
    expect(activateLicense).not.toHaveBeenCalled();
  });

  it("asks once when both delivery paths carry the same click", async () => {
    // macOS can deliver one cold-start link through both startup and Rust events.
    activateLicense.mockResolvedValue({ tier: "core", isActive: true });
    getCurrentDeepLinks.mockResolvedValue(["sitecmd://activate?key=test-fixture-key-001"]); // gitleaks:allow

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));
    await waitFor(() => expect(confirmLinkLicenseActivation).toHaveBeenCalledOnce());

    emitDeepLink({ key: "test-fixture-key-001" }); // gitleaks:allow

    await waitFor(() => expect(activateLicense).toHaveBeenCalledOnce());
    expect(confirmLinkLicenseActivation).toHaveBeenCalledOnce();
  });

  it("calls activateLicense with the deep-link key and surfaces a success toast", async () => {
    activateLicense.mockResolvedValue({ tier: "core", isActive: true });

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-key-001" }); // gitleaks:allow

    await waitFor(() => expect(activateLicense).toHaveBeenCalledWith("test-fixture-key-001"));
    await waitFor(() => {
      expect(toastSuccess).toHaveBeenCalledWith(
        "Plus active",
        expect.stringContaining("License activated"),
      );
    });
    expect(toastError).not.toHaveBeenCalled();
  });

  it("uses the Professional label when the activated license is a Pro tier", async () => {
    activateLicense.mockResolvedValue({ tier: "pro", isActive: true });

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-pro-key-001" });

    await waitFor(() => {
      expect(toastSuccess).toHaveBeenCalledWith(
        "Professional active",
        expect.stringContaining("License activated"),
      );
    });
  });

  it("answers a resolved-but-inactive activation with the honest warning, never success", async () => {
    activateLicense.mockResolvedValue({ tier: "free", isActive: false });

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-key-001" }); // gitleaks:allow

    await waitFor(() => {
      expect(toastWarning).toHaveBeenCalledWith(
        "License checked",
        expect.stringContaining("Free plan"),
      );
    });
    expect(toastSuccess).not.toHaveBeenCalled();
    expect(toastError).not.toHaveBeenCalled();
  });

  it("shows an error toast and does not call activateLicense when the payload is missing the key", async () => {
    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "" });

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(activateLicense).not.toHaveBeenCalled();
  });

  it("rejects pathological key lengths without calling activateLicense", async () => {
    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "a".repeat(257) });

    await waitFor(() =>
      expect(toastError).toHaveBeenCalledWith(
        "Couldn't activate license",
        expect.stringContaining("malformed"),
      ),
    );
    expect(activateLicense).not.toHaveBeenCalled();
  });

  it("formats a structured refusal instead of toasting the raw JSON payload", async () => {
    activateLicense.mockRejectedValueOnce(new Error('{"code":"changed_during_activation"}'));

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "RACED-KEY" });

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith(
        "License activation failed",
        expect.stringContaining("nothing was replaced"),
      );
    });
    expect(toastError).not.toHaveBeenCalledWith(
      "License activation failed",
      expect.stringContaining('{"code"'),
    );
  });

  it("stays silent when the user declined the replacement dialog", async () => {
    activateLicense.mockRejectedValueOnce(new Error('{"code":"cancelled"}'));

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "DECLINED-KEY" });

    await waitFor(() => expect(activateLicense).toHaveBeenCalled());
    expect(toastError).not.toHaveBeenCalled();
    expect(toastSuccess).not.toHaveBeenCalled();
  });

  it("maps a structured refusal to the written copy, never the raw string", async () => {
    activateLicense.mockRejectedValueOnce(new Error('{"code":"not_found"}'));

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "BAD-KEY" });

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith("License activation failed", expect.any(String));
    });
    const [, description] = toastError.mock.calls[0] as [string, string];
    expect(description).not.toBe('{"code":"not_found"}');
  });

  it("never welcomes the installed license when the command never ran", async () => {
    activateLicense.mockRejectedValueOnce(
      new Error("Privileged external-connectors bridge window did not become ready: timeout"),
    );
    refreshLicense.mockResolvedValueOnce({ isActive: true, tier: "pro" });

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "GOOD-KEY" });

    await waitFor(() => expect(refreshLicense).toHaveBeenCalled());
    expect(toastSuccess).not.toHaveBeenCalled();
    expect(toastError).not.toHaveBeenCalled();
    expect(toastWarning).toHaveBeenCalledWith(
      "Activation could not be confirmed",
      expect.stringContaining("enter your key there"),
    );
  });

  it("never reports a client-side timeout as either success or failure", async () => {
    const timedOut = Object.assign(new Error("That action took too long to finish."), {
      command: "activate_license",
      scope: "external-connectors",
      timeoutMs: 180_000,
    });
    activateLicense.mockRejectedValueOnce(timedOut);
    refreshLicense.mockResolvedValueOnce({ isActive: true, tier: "pro" });

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "GOOD-KEY" });

    await waitFor(() => expect(refreshLicense).toHaveBeenCalled());
    expect(toastSuccess).not.toHaveBeenCalled();
    expect(toastError).not.toHaveBeenCalled();
    expect(toastWarning).toHaveBeenCalledWith(
      "Activation could not be confirmed",
      expect.any(String),
    );
  });

  it("activates from the startup URL when the purchase link launched the app", async () => {
    activateLicense.mockResolvedValue({ tier: "core", isActive: true });
    getCurrentDeepLinks.mockResolvedValue([
      "sitecmd://activate?key=test-fixture-key-001", // gitleaks:allow
    ]);

    renderHook(() => useLicenseActivateDeepLink());

    await waitFor(() => expect(activateLicense).toHaveBeenCalledWith("test-fixture-key-001"));
    await waitFor(() => {
      expect(toastSuccess).toHaveBeenCalledWith(
        "Plus active",
        expect.stringContaining("License activated"),
      );
    });
  });

  it("handles one click once when the startup URL and the Rust event both deliver it", async () => {
    activateLicense.mockResolvedValue({ tier: "core", isActive: true });
    getCurrentDeepLinks.mockResolvedValue([
      "sitecmd://activate?key=test-fixture-key-001", // gitleaks:allow
    ]);

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(activateLicense).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-key-001" }); // gitleaks:allow

    await waitFor(() => expect(toastSuccess).toHaveBeenCalled());
    expect(activateLicense).toHaveBeenCalledTimes(1);
  });

  it("still handles a different key arriving after the startup one", async () => {
    // The dedupe is per key, not per session: an upgrade link clicked right
    // after a first activation is a new request, not a duplicate.
    activateLicense.mockResolvedValue({ tier: "core", isActive: true });
    getCurrentDeepLinks.mockResolvedValue([
      "sitecmd://activate?key=test-fixture-key-001", // gitleaks:allow
    ]);

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(activateLicense).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "test-fixture-pro-key-002" });

    await waitFor(() => expect(activateLicense).toHaveBeenCalledTimes(2));
    expect(activateLicense).toHaveBeenLastCalledWith("test-fixture-pro-key-002");
  });

  it("ignores startup URLs that are not activation links", async () => {
    getCurrentDeepLinks.mockResolvedValue(["sitecmd://open?page=settings"]);

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    expect(activateLicense).not.toHaveBeenCalled();
    expect(toastError).not.toHaveBeenCalled();
  });

  it("covers the upgrade case, where no key field is on screen", async () => {
    // Existing subscribers may need refresh guidance when no key field is visible.
    const timedOut = Object.assign(new Error("That action took too long to finish."), {
      command: "activate_license",
      scope: "external-connectors",
      timeoutMs: 180_000,
    });
    activateLicense.mockRejectedValueOnce(timedOut);

    renderHook(() => useLicenseActivateDeepLink());
    await waitFor(() => expect(listeners.get(LICENSE_ACTIVATE_EVENT)?.size).toBeGreaterThan(0));

    emitDeepLink({ key: "GOOD-KEY" });

    await waitFor(() => expect(toastWarning).toHaveBeenCalled());
    const [, body] = toastWarning.mock.calls.at(-1) ?? [];
    expect(body).toContain("enter your key there");
    expect(body).toContain("Refresh License");
  });
});
