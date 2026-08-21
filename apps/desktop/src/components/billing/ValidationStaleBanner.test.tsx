import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ValidationStaleBanner } from "./ValidationStaleBanner";
import type { ValidationWarning } from "@/hooks/useTier";
import { SUPPORT_EMAIL } from "@/lib/support";

let mockLicenseInfo: {
  planName: string;
  validationWarning: ValidationWarning;
};

const mockRefreshLicense = vi.fn<() => Promise<{ validationWarning: ValidationWarning } | null>>();
const mockToastError = vi.fn();

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({ licenseInfo: mockLicenseInfo, refreshLicense: mockRefreshLicense }),
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    toast: vi.fn(),
    success: vi.fn(),
    error: mockToastError,
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));

describe("ValidationStaleBanner", () => {
  beforeEach(() => {
    mockLicenseInfo = { planName: "Plus", validationWarning: "none" };
    mockRefreshLicense.mockReset();
    mockToastError.mockReset();
  });

  afterEach(() => {
    mockLicenseInfo = { planName: "Plus", validationWarning: "none" };
  });

  it("renders nothing when validation_warning is 'none'", () => {
    const { container } = render(<ValidationStaleBanner />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the soft banner when validation_warning is 'stale'", () => {
    mockLicenseInfo = { planName: "Plus", validationWarning: "stale" };
    render(<ValidationStaleBanner />);
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent(/Couldn't reach the license server/i);
    expect(alert).toHaveTextContent(/Plus/);
    expect(alert).toHaveTextContent(/Connected service access is using cached validation/i);
    expect(alert).toHaveTextContent(/Local scans and fixes are unaffected/i);
    expect(alert).not.toHaveTextContent(/will pause in 24 hours/i);
    expect(alert.className).toContain("validation-stale-banner");
    expect(alert.className).not.toContain("validation-stale-banner--final");
  });

  it("renders the loud final-warning banner when validation_warning is 'stale_final_warning'", () => {
    mockLicenseInfo = { planName: "Pro", validationWarning: "stale_final_warning" };
    render(<ValidationStaleBanner />);
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent(/over 7 days/i);
    expect(alert).toHaveTextContent(/Professional connected service access/i);
    expect(alert).toHaveTextContent(/will pause in 24 hours/i);
    expect(alert).toHaveTextContent(/Local scans and fixes remain available/i);
    expect(alert).toHaveTextContent(SUPPORT_EMAIL);
    expect(alert.className).toContain("validation-stale-banner--final");
  });

  it("uses the licenseInfo.plan_name in the message body", () => {
    mockLicenseInfo = { planName: "Pro", validationWarning: "stale" };
    render(<ValidationStaleBanner />);
    expect(screen.getByRole("alert")).toHaveTextContent(/your Professional license/);
  });

  it("renders the re-enter-your-key banner when validation_warning is 'key_unreadable'", () => {
    mockLicenseInfo = { planName: "Plus", validationWarning: "key_unreadable" };
    render(<ValidationStaleBanner />);
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent(/Couldn't read your Plus license key/i);
    expect(alert).toHaveTextContent(/Connected service access may be unavailable/i);
    expect(alert).toHaveTextContent(/local scans and fixes are unaffected/i);
    expect(alert).toHaveTextContent(/Re-enter your license key in Settings/i);
    // The message must point at Settings, not at the network.
    expect(alert).not.toHaveTextContent(/Check your connection/i);
    expect(alert.className).not.toContain("validation-stale-banner--final");
    // Retrying revalidation cannot fix a locally unreadable key.
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
  });

  it("renders the reactivate banner when validation_warning is 'instance_deactivated'", () => {
    mockLicenseInfo = { planName: "Free", validationWarning: "instance_deactivated" };
    render(<ValidationStaleBanner />);
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent(/license activation was removed/i);
    expect(alert).toHaveTextContent(/Re-enter your license key in Settings/i);
    expect(alert).toHaveTextContent(/Local scans and saved guidance still work/i);
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
  });

  it("offers a Retry button on both banner intensities", () => {
    mockLicenseInfo = { planName: "Plus", validationWarning: "stale" };
    const { unmount } = render(<ValidationStaleBanner />);
    expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled();
    unmount();

    mockLicenseInfo = { planName: "Pro", validationWarning: "stale_final_warning" };
    render(<ValidationStaleBanner />);
    expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled();
  });

  it("disables the button and forces a real revalidation while the retry is in flight", async () => {
    mockLicenseInfo = { planName: "Plus", validationWarning: "stale" };
    let resolveRefresh!: (info: { validationWarning: ValidationWarning } | null) => void;
    mockRefreshLicense.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveRefresh = resolve;
        }),
    );
    render(<ValidationStaleBanner />);

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    expect(mockRefreshLicense).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Retrying..." })).toBeDisabled();

    resolveRefresh({ validationWarning: "none" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled());
  });

  it("raises an error toast and re-enables Retry when revalidation keeps failing", async () => {
    mockLicenseInfo = { planName: "Plus", validationWarning: "stale" };
    mockRefreshLicense.mockResolvedValue({ validationWarning: "stale" });
    render(<ValidationStaleBanner />);

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() =>
      expect(mockToastError).toHaveBeenCalledWith("Still couldn't reach the license server."),
    );
    expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled();
    // Failure feedback is a toast, never text crammed into the banner.
    expect(screen.getByRole("alert")).not.toHaveTextContent(/still couldn't/i);
  });

  it("raises the re-enter-key toast when a retry discovers the key is unreadable", async () => {
    // Network comes back mid-outage but the key is locally gone: the toast
    // must send the user to Settings, not blame the connection.
    mockLicenseInfo = { planName: "Plus", validationWarning: "stale" };
    mockRefreshLicense.mockResolvedValue({ validationWarning: "key_unreadable" });
    render(<ValidationStaleBanner />);

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() =>
      expect(mockToastError).toHaveBeenCalledWith(
        "Your license key couldn't be read. Re-enter it in Settings.",
      ),
    );
  });

  it("treats a rejected-into-null refresh as a failure, not a success", async () => {
    // refreshLicense catches command errors and resolves null; the banner
    // must read that as "still failing" rather than silently going idle.
    mockLicenseInfo = { planName: "Plus", validationWarning: "stale" };
    mockRefreshLicense.mockResolvedValue(null);
    render(<ValidationStaleBanner />);

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() =>
      expect(mockToastError).toHaveBeenCalledWith("Still couldn't reach the license server."),
    );
  });

  it("does not raise the failure toast when the retry clears the warning", async () => {
    mockLicenseInfo = { planName: "Plus", validationWarning: "stale" };
    mockRefreshLicense.mockResolvedValue({ validationWarning: "none" });
    render(<ValidationStaleBanner />);

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    // On success the tier context updates and unmounts the banner; the
    // component itself must not flash a failure toast in the meantime.
    await waitFor(() => expect(mockRefreshLicense).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled());
    expect(mockToastError).not.toHaveBeenCalled();
  });
});
