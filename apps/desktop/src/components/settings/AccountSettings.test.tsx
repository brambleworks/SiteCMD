import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { withQueryClient } from "@/test-utils/query-client";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AccountSettings } from "./AccountSettings";

const {
  openUrlMock,
  toastErrorMock,
  toastSuccessMock,
  toastWarningMock,
  activateLicenseMock,
  deactivateLicenseMock,
  refreshLicenseMock,
  retryCatalogRefreshMock,
  getCatalogStatusMock,
  tierState,
} = vi.hoisted(() => ({
  openUrlMock: vi.fn(),
  toastErrorMock: vi.fn(),
  toastSuccessMock: vi.fn(),
  toastWarningMock: vi.fn(),
  activateLicenseMock: vi.fn(),
  deactivateLicenseMock: vi.fn(),
  refreshLicenseMock: vi.fn(),
  retryCatalogRefreshMock: vi.fn(),
  getCatalogStatusMock: vi.fn(),
  tierState: {
    tier: "free",
    licenseInfo: {
      tier: "free",
      status: "none",
      planName: "Free",
      billingInterval: null as "monthly" | "yearly" | null,
      isActive: false,
      expiresAt: null,
      checkoutUrls: {
        core: "https://shop.sitecmd.com/checkout/buy/core",
        pro: "https://shop.sitecmd.com/checkout/buy/pro",
        coreMonthly: "",
        coreAnnual: "",
        proMonthly: "",
        proAnnual: "",
      },
      customerPortalUrl: "",
    },
  },
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    ...tierState,
    isLoading: false,
    activateLicense: activateLicenseMock,
    deactivateLicense: deactivateLicenseMock,
    refreshLicense: refreshLicenseMock,
  }),
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: toastSuccessMock,
    error: toastErrorMock,
    warning: toastWarningMock,
    info: vi.fn(),
  }),
}));

vi.mock("@/lib/open-url", () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...args),
}));

vi.mock("@/lib/commands", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/commands")>()),
  getCatalogStatus: (...args: unknown[]) => getCatalogStatusMock(...args),
  retryCatalogRefresh: (...args: unknown[]) =>
    (retryCatalogRefreshMock(...args) as Promise<void> | undefined) ?? Promise.resolve(),
}));

describe("AccountSettings commercial boundary", () => {
  beforeEach(() => {
    openUrlMock.mockReset();
    toastErrorMock.mockReset();
    toastSuccessMock.mockReset();
    toastWarningMock.mockReset();
    activateLicenseMock.mockReset();
    deactivateLicenseMock.mockReset();
    refreshLicenseMock.mockReset();
    retryCatalogRefreshMock.mockReset();
    getCatalogStatusMock.mockReset();
    getCatalogStatusMock.mockResolvedValue({
      active: true,
      catalogVersion: "2026-07-28",
      publishedAt: "2026-07-28T18:00:00.000Z",
      endpointConfigured: true,
    });
    tierState.tier = "free";
    tierState.licenseInfo.tier = "free";
    tierState.licenseInfo.status = "none";
    tierState.licenseInfo.planName = "Free";
    tierState.licenseInfo.billingInterval = null;
    tierState.licenseInfo.isActive = false;
    tierState.licenseInfo.expiresAt = null;
    tierState.licenseInfo.customerPortalUrl = "";
    tierState.licenseInfo.checkoutUrls = {
      core: "https://shop.sitecmd.com/checkout/buy/core",
      pro: "https://shop.sitecmd.com/checkout/buy/pro",
      coreMonthly: "",
      coreAnnual: "",
      proMonthly: "",
      proAnnual: "",
    };
  });

  it("offers connected beta access instead of an invented paid plan", () => {
    render(<AccountSettings />, { wrapper: withQueryClient() });

    fireEvent.click(screen.getByRole("button", { name: /Request beta access/i }));
    expect(openUrlMock).toHaveBeenCalledWith("https://sitecmd.com/contact");
    expect(screen.queryByRole("button", { name: /Get Plus/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Get Professional/i })).not.toBeInTheDocument();
  });

  it("states that the full local workbench is free and the connected beta is free", () => {
    render(<AccountSettings />, { wrapper: withQueryClient() });

    expect(screen.getByText(/desktop workbench is free and complete/i)).toBeInTheDocument();
    expect(screen.getByText(/^free during the beta$/i)).toBeInTheDocument();
    expect(screen.queryByText(/\$\d+/)).not.toBeInTheDocument();
  });

  it("shows the terms and privacy policy beside license activation", () => {
    render(<AccountSettings />, { wrapper: withQueryClient() });

    expect(screen.getByRole("link", { name: "Terms of Service" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Privacy Policy" })).toBeInTheDocument();
  });

  it("does not expose a checkout even when legacy checkout URLs are configured", () => {
    render(<AccountSettings />, { wrapper: withQueryClient() });

    expect(screen.queryByRole("button", { name: /Get Plus/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/\/mo|\/yr/)).not.toBeInTheDocument();
  });

  it("never announces success off state when the failure was not the backend's answer", async () => {
    activateLicenseMock.mockRejectedValue(new Error("privileged command timed out"));
    refreshLicenseMock.mockResolvedValue({
      ...tierState.licenseInfo,
      tier: "pro",
      status: "active",
      planName: "Pro",
      isActive: true,
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.change(screen.getByPlaceholderText(/X{4}/), { target: { value: "KEY-123" } });
    fireEvent.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalled());
    expect(toastSuccessMock).not.toHaveBeenCalled();
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(toastWarningMock).toHaveBeenCalledWith(
      "Activation could not be confirmed",
      expect.stringContaining("cannot tell whether"),
    );
  });

  it("reports a structured refusal as a failure, whatever state says", async () => {
    activateLicenseMock.mockRejectedValue(new Error('{"code":"invalid_key"}'));
    refreshLicenseMock.mockResolvedValue(null);

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.change(screen.getByPlaceholderText(/X{4}/), { target: { value: "BAD-KEY" } });
    fireEvent.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() => expect(toastErrorMock).toHaveBeenCalled());
    expect(toastErrorMock).toHaveBeenCalledWith("Activation failed", expect.any(String));
    expect(toastSuccessMock).not.toHaveBeenCalled();
  });

  it("shows a raced activation its own message instead of reconciling into a false success", async () => {
    activateLicenseMock.mockRejectedValue(new Error('{"code":"changed_during_activation"}'));
    refreshLicenseMock.mockResolvedValue({
      ...tierState.licenseInfo,
      tier: "core",
      planName: "Plus",
      isActive: true,
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.change(screen.getByPlaceholderText(/X{4}/), { target: { value: "KEY-123" } });
    fireEvent.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() =>
      expect(toastErrorMock).toHaveBeenCalledWith(
        "Activation failed",
        expect.stringContaining("nothing was replaced"),
      ),
    );
    expect(toastSuccessMock).not.toHaveBeenCalled();
    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalled());
    expect(toastSuccessMock).not.toHaveBeenCalled();
  });

  it("a cancelled replacement dialog announces nothing", async () => {
    activateLicenseMock.mockRejectedValue(new Error('{"code":"cancelled"}'));
    refreshLicenseMock.mockResolvedValue({
      ...tierState.licenseInfo,
      isActive: true,
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.change(screen.getByPlaceholderText(/X{4}/), { target: { value: "KEY-123" } });
    fireEvent.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() => expect(activateLicenseMock).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByRole("button", { name: "Activate" })).toBeEnabled());
    expect(toastSuccessMock).not.toHaveBeenCalled();
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(refreshLicenseMock).not.toHaveBeenCalled();
  });

  it("a dead replacement dialog reports failure instead of welcoming the old license", async () => {
    // A failed replacement dialog is conclusive failure even if the existing
    // license remains active.
    activateLicenseMock.mockRejectedValue(new Error('{"code":"incomplete"}'));
    refreshLicenseMock.mockResolvedValue({
      ...tierState.licenseInfo,
      tier: "core",
      planName: "Plus",
      isActive: true,
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.change(screen.getByPlaceholderText(/X{4}/), { target: { value: "KEY-123" } });
    fireEvent.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() =>
      expect(toastErrorMock).toHaveBeenCalledWith(
        "Activation failed",
        expect.stringContaining("stopped before it could complete"),
      ),
    );
    expect(toastSuccessMock).not.toHaveBeenCalled();
    // The convergence read must not reinterpret this failed attempt as success.
    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalled());
    expect(toastSuccessMock).not.toHaveBeenCalled();
  });

  it("a client-side timeout claims neither success nor failure", async () => {
    // A bridge timeout is inconclusive because the native command may still run.
    activateLicenseMock.mockRejectedValue(
      Object.assign(new Error("That action took too long to finish."), {
        command: "activate_license",
        scope: "external-connectors",
        timeoutMs: 180_000,
      }),
    );
    refreshLicenseMock.mockResolvedValue({
      ...tierState.licenseInfo,
      tier: "core",
      planName: "Plus",
      isActive: true,
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.change(screen.getByPlaceholderText(/X{4}/), { target: { value: "KEY-123" } });
    fireEvent.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalled());
    expect(toastSuccessMock).not.toHaveBeenCalled();
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(toastWarningMock).toHaveBeenCalledWith(
      "Activation could not be confirmed",
      expect.stringContaining("enter the key again"),
    );
  });

  it("an unrecognized provider refusal shows the provider's words, not a welcome", async () => {
    activateLicenseMock.mockRejectedValue(
      new Error('{"code":"provider_refused","message":"This license key has been disabled."}'),
    );
    refreshLicenseMock.mockResolvedValue({
      ...tierState.licenseInfo,
      tier: "core",
      planName: "Plus",
      isActive: true,
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.change(screen.getByPlaceholderText(/X{4}/), { target: { value: "KEY-123" } });
    fireEvent.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() =>
      expect(toastErrorMock).toHaveBeenCalledWith(
        "Activation failed",
        expect.stringContaining("This license key has been disabled."),
      ),
    );
    expect(toastSuccessMock).not.toHaveBeenCalled();
  });

  it("keeps one red in the deactivate card, on the action not the sentence", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;

    render(<AccountSettings />, { wrapper: withQueryClient() });
    const deactivate = await screen.findByRole("button", {
      name: "Deactivate license on this device",
    });
    expect(deactivate).toHaveClass("text-destructive");

    fireEvent.click(deactivate);

    // The sentence stays muted: a second red said the same thing twice, and
    // --destructive is 2.27:1 on --card in the shipped dark theme, under AA.
    const warning = await screen.findByText(/This will unlink your license/);
    expect(warning).toHaveClass("text-body-muted");
    expect(warning.className).not.toMatch(/text-(?:severity|score)-|text-destructive/);
  });

  it("converges the display when deactivation reports failure", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    deactivateLicenseMock.mockRejectedValue(
      new Error("This machine was unlinked and its activations released, but the license key"),
    );

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.click(
      await screen.findByRole("button", { name: "Deactivate license on this device" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() =>
      expect(toastErrorMock).toHaveBeenCalledWith("Deactivation failed", expect.any(String)),
    );
    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalled());
  });

  it("a deactivation timeout claims nothing but still converges the panel", async () => {
    // A client timeout cannot prove whether the native confirmation completed.
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    deactivateLicenseMock.mockRejectedValue(
      Object.assign(new Error("That action took too long to finish."), {
        command: "deactivate_license",
        scope: "data-admin",
        timeoutMs: 180_000,
      }),
    );

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.click(
      await screen.findByRole("button", { name: "Deactivate license on this device" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() =>
      expect(toastWarningMock).toHaveBeenCalledWith(
        "Deactivation could not be confirmed",
        expect.stringContaining("cannot say yet whether"),
      ),
    );
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(toastSuccessMock).not.toHaveBeenCalled();
    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: "Confirm" })).toBeNull();
    const [, body] = toastWarningMock.mock.calls.at(-1) ?? [];
    expect(body).toContain("Refresh License");
    expect(body).not.toContain("updates on its own");
    expect(screen.queryByRole("button", { name: "Refresh License" })).not.toBeNull();
  });

  it("a degraded refusal reads as unresolved, not as the service's decline", async () => {
    // An unknown 4xx may come from a proxy, so it is not a service verdict.
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    getCatalogStatusMock.mockResolvedValue({
      active: false,
      credentialBlock: { code: "refused" },
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });

    await screen.findByText(/did not get a clear answer/);
    expect(screen.queryByText(/declined to issue this machine/)).toBeNull();
    expect(screen.queryByText(/Re-enter your license key/)).toBeNull();
  });

  it("a lapsed subscription is sent to billing, not to re-enter a key", async () => {
    // A billing lapse requires billing guidance, not license-key re-entry.
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    getCatalogStatusMock.mockResolvedValue({
      active: false,
      credentialBlock: { code: "subscription_inactive" },
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });

    await screen.findByText(/subscription is not currently active/);
    expect(screen.queryByText(/entering it again fixes it/)).toBeNull();
    expect(screen.queryByText(/declined to issue this machine/)).toBeNull();
    // The remedy it names has to be on screen.
    expect(screen.queryByRole("button", { name: "Manage Billing" })).not.toBeNull();
  });

  it("a build with no catalog endpoint says so instead of promising a download", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    getCatalogStatusMock.mockResolvedValue({ active: false, endpointConfigured: false });

    render(<AccountSettings />, { wrapper: withQueryClient() });

    await screen.findByText(/packaged without guide-catalog access/);
    expect(screen.queryByText(/downloads automatically in the background/)).toBeNull();
  });

  it("a configured build that has not downloaded yet still says the catalog is coming", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    getCatalogStatusMock.mockResolvedValue({ active: false, endpointConfigured: true });

    render(<AccountSettings />, { wrapper: withQueryClient() });

    await screen.findByText(/downloads automatically in the background/);
    expect(screen.queryByText(/packaged without guide-catalog access/)).toBeNull();
  });

  it("names the code on a refusal only support can resolve", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    getCatalogStatusMock.mockResolvedValue({
      active: false,
      credentialBlock: { code: "wrong_store" },
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });

    await screen.findByText(/declined to issue this machine/);
    expect(screen.getByText(/wrong_store/)).toBeTruthy();
  });

  it("an activation that resolves without an active license does not claim a welcome", async () => {
    activateLicenseMock.mockResolvedValue({
      ...tierState.licenseInfo,
      planName: "Free",
      isActive: false,
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.change(screen.getByPlaceholderText(/X{4}/), { target: { value: "KEY-123" } });
    fireEvent.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() =>
      expect(toastWarningMock).toHaveBeenCalledWith("License checked", expect.any(String)),
    );
    expect(toastSuccessMock).not.toHaveBeenCalled();
    const [, body] = toastWarningMock.mock.calls.at(-1) ?? [];
    expect(body).not.toContain("Refresh License");
    expect(screen.queryByRole("button", { name: "Refresh License" })).toBeNull();
  });

  it("does not expose the retired SiteCMD-managed trial flow", () => {
    render(<AccountSettings />, { wrapper: withQueryClient() });

    expect(screen.queryByText(/Start free trial/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/activation code/i)).not.toBeInTheDocument();
  });

  it("keeps connected-service details collapsed for an existing Plus license", () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.billingInterval = "monthly";
    tierState.licenseInfo.isActive = true;

    render(<AccountSettings />, { wrapper: withQueryClient() });

    expect(document.querySelector(".plan-card-name")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "View Details" }));

    expect(document.querySelector(".plan-card-name")).toHaveTextContent("SiteCMD Connect");
    expect(screen.queryByText("Professional")).not.toBeInTheDocument();
    expect(screen.getByText("PLUS MONTHLY")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connected access licensed" })).toBeDisabled();
    expect(screen.queryByText(/\/mo|\/yr/)).not.toBeInTheDocument();
  });

  it("states catalog freshness once instead of stuttering the date as a version", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;

    render(<AccountSettings />, { wrapper: withQueryClient() });

    const expectedDate = new Date("2026-07-28T18:00:00.000Z").toLocaleDateString();
    await waitFor(() =>
      expect(screen.getByText(new RegExp(`Guides updated ${expectedDate}`))).toBeInTheDocument(),
    );
    expect(screen.queryByText(/, published/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Version 2026-07-28/)).not.toBeInTheDocument();
  });

  it("Refresh License also retries the catalog credential", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    refreshLicenseMock.mockResolvedValue({ ...tierState.licenseInfo });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.click(await screen.findByRole("button", { name: "Refresh License" }));

    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalled());
    await waitFor(() => expect(retryCatalogRefreshMock).toHaveBeenCalled());
  });

  it("reports a keychain remnant as a completed unlink, not a failed one", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    deactivateLicenseMock.mockRejectedValue(
      new Error(
        "unlinked_with_keychain_remnant: This machine was unlinked and its activations released, but the license key could not be removed from the keychain (ACL denied).",
      ),
    );

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.click(
      await screen.findByRole("button", { name: "Deactivate license on this device" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() =>
      expect(toastWarningMock).toHaveBeenCalledWith(
        "License unlinked, with one thing left over",
        expect.stringContaining("could not be removed from the keychain"),
      ),
    );
    expect(toastErrorMock).not.toHaveBeenCalled();
    // The marker is machinery, not prose, and must never reach the user.
    expect(toastWarningMock).not.toHaveBeenCalledWith(
      expect.anything(),
      expect.stringContaining("unlinked_with_keychain_remnant"),
    );
    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: "Confirm" })).toBeNull();
  });

  it("declining the native deactivate dialog stays silent and keeps the confirm open", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    deactivateLicenseMock.mockRejectedValue(new Error('{"code":"cancelled"}'));

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.click(
      await screen.findByRole("button", { name: "Deactivate license on this device" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() => expect(deactivateLicenseMock).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByRole("button", { name: "Confirm" })).toBeEnabled());
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(toastSuccessMock).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Confirm" })).toBeInTheDocument();

    // A real failure still surfaces, verbatim.
    deactivateLicenseMock.mockRejectedValue(new Error("No active license to deactivate"));
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    await waitFor(() =>
      expect(toastErrorMock).toHaveBeenCalledWith(
        "Deactivation failed",
        expect.stringContaining("No active license"),
      ),
    );
  });

  it("a failing catalog retry surfaces instead of dying unhandled", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    refreshLicenseMock.mockResolvedValue({ ...tierState.licenseInfo });
    retryCatalogRefreshMock.mockImplementation(() => {
      throw new Error("ipc unavailable");
    });

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.click(await screen.findByRole("button", { name: "Refresh License" }));

    await waitFor(() =>
      expect(toastErrorMock).toHaveBeenCalledWith(
        "Refresh failed",
        expect.stringContaining("ipc unavailable"),
      ),
    );
  });

  it("preserves a Pro license without advertising retired public plans", () => {
    tierState.tier = "pro";
    tierState.licenseInfo.tier = "pro";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Pro";
    tierState.licenseInfo.billingInterval = "yearly";
    tierState.licenseInfo.isActive = true;

    render(<AccountSettings />, { wrapper: withQueryClient() });

    fireEvent.click(screen.getByRole("button", { name: "View Details" }));

    expect(document.querySelector(".plan-card-name")).toHaveTextContent("SiteCMD Connect");
    expect(screen.getByText("PROFESSIONAL YEARLY")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connected access licensed" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: /Get Plus|Get Professional/ })).toBeNull();
    expect(document.querySelector(".settings-inline-card")).not.toHaveTextContent("Professional");
  });

  it("the Refresh License gesture forces a live check", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    refreshLicenseMock.mockResolvedValue(null);

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.click(await screen.findByRole("button", { name: "Refresh License" }));

    await waitFor(() => expect(refreshLicenseMock).toHaveBeenCalledWith({ force: true }));
    // The cap banner's remedy says this gesture retries the catalog
    // credential too, so the click must kick both.
    expect(retryCatalogRefreshMock).toHaveBeenCalled();
  });

  it("one refresh at a time: the button disables until the gesture settles", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    let releaseRefresh: (value: null) => void = () => {};
    refreshLicenseMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          releaseRefresh = resolve;
        }),
    );

    render(<AccountSettings />, { wrapper: withQueryClient() });
    const button = await screen.findByRole("button", { name: "Refresh License" });
    fireEvent.click(button);

    await waitFor(() => expect(button).toBeDisabled());
    fireEvent.click(button);
    expect(refreshLicenseMock).toHaveBeenCalledTimes(1);

    releaseRefresh(null);
    await waitFor(() => expect(button).toBeEnabled());
  });

  it("a catalog rejection neither releases the guard early nor passes silently", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    let releaseRefresh: (value: unknown) => void = () => {};
    refreshLicenseMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          releaseRefresh = resolve;
        }),
    );
    retryCatalogRefreshMock.mockRejectedValue(new Error("ipc boundary"));

    render(<AccountSettings />, { wrapper: withQueryClient() });
    const button = await screen.findByRole("button", { name: "Refresh License" });
    fireEvent.click(button);

    // The catalog kick has already rejected; the validation has not settled.
    await waitFor(() => expect(retryCatalogRefreshMock).toHaveBeenCalled());
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(refreshLicenseMock).toHaveBeenCalledTimes(1);

    releaseRefresh(tierState.licenseInfo);
    await waitFor(() => expect(button).toBeEnabled());
    expect(toastErrorMock).toHaveBeenCalledWith("Refresh failed", expect.stringContaining("ipc"));
  });

  it("a live check that could not complete says so instead of stopping the spinner silently", async () => {
    tierState.tier = "core";
    tierState.licenseInfo.tier = "core";
    tierState.licenseInfo.status = "active";
    tierState.licenseInfo.planName = "Plus";
    tierState.licenseInfo.isActive = true;
    refreshLicenseMock.mockResolvedValue(null);

    render(<AccountSettings />, { wrapper: withQueryClient() });
    fireEvent.click(await screen.findByRole("button", { name: "Refresh License" }));

    await waitFor(() =>
      expect(toastErrorMock).toHaveBeenCalledWith(
        "Could not verify the license",
        expect.any(String),
      ),
    );
  });
});
