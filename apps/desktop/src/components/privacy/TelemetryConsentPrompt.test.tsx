import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  __resetTelemetryForTests,
  __setTelemetryConsentAuthorityForTests,
  buildTelemetryPreview,
  useTelemetryConsent,
} from "@/lib/telemetry";
import * as telemetry from "@/lib/telemetry";
import { TelemetryConsentPrompt } from "./TelemetryConsentPrompt";

function ConsentStateProbe() {
  const consent = useTelemetryConsent();
  return (
    <output aria-label="telemetry consent">
      {JSON.stringify({
        usageAnalytics: consent.usageAnalytics,
        crashReports: consent.crashReports,
        promptStatus: consent.promptStatus,
      })}
    </output>
  );
}

describe("TelemetryConsentPrompt", () => {
  beforeEach(() => {
    localStorage.clear();
    __resetTelemetryForTests();
    __setTelemetryConsentAuthorityForTests({
      get: async () => ({
        usageAnalytics: false,
        crashReports: false,
        consentVersion: 1,
        updatedAt: null,
      }),
      set: async ({ args }) => ({
        ...args,
        consentVersion: 1,
        updatedAt: new Date().toISOString(),
      }),
    });
  });

  it("keeps usage analytics and crash reports off until the user opts in", async () => {
    render(
      <>
        <TelemetryConsentPrompt />
        <ConsentStateProbe />
      </>,
    );

    expect(screen.getByRole("switch", { name: "Usage analytics" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
    expect(screen.getByRole("switch", { name: "Crash and error reports" })).toHaveAttribute(
      "aria-checked",
      "false",
    );

    fireEvent.click(screen.getByRole("button", { name: "Save Choices" }));

    await waitFor(() => {
      expect(screen.queryByText("Help improve SiteCMD")).not.toBeInTheDocument();
    });
    expect(screen.getByLabelText("telemetry consent")).toHaveTextContent(
      '{"usageAnalytics":false,"crashReports":false,"promptStatus":"saved"}',
    );
    expect(buildTelemetryPreview()).toContain("Usage analytics: off");
    expect(buildTelemetryPreview()).toContain("Crash and error reports: off");
  });

  it("'Keep Off' explicitly saves both signals as disabled", async () => {
    render(
      <>
        <TelemetryConsentPrompt />
        <ConsentStateProbe />
      </>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Keep Off" }));

    await waitFor(() => {
      expect(screen.queryByText("Help improve SiteCMD")).not.toBeInTheDocument();
    });
    expect(screen.getByLabelText("telemetry consent")).toHaveTextContent(
      '{"usageAnalytics":false,"crashReports":false,"promptStatus":"saved"}',
    );
  });

  it("surfaces a retry-friendly error when saving the consent choice fails", async () => {
    const spy = vi
      .spyOn(telemetry, "setTelemetryConsent")
      .mockRejectedValueOnce(new Error("storage quota exceeded"));

    try {
      render(<TelemetryConsentPrompt />);

      fireEvent.click(screen.getByRole("button", { name: "Keep Off" }));

      const errorMessage = await screen.findByRole("alert");
      expect(errorMessage).toHaveTextContent(/Couldn't save telemetry choice/i);
      expect(errorMessage).toHaveTextContent(/storage quota exceeded/i);
      expect(errorMessage).toHaveTextContent(/Try again/i);
      // Modal stays open so the user can retry; saving spinner is cleared.
      expect(screen.getByText("Help improve SiteCMD")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Keep Off" })).not.toBeDisabled();
    } finally {
      spy.mockRestore();
    }
  });
});
