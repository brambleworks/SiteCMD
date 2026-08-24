import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

// Mock the invoke boundary so the real fetchPageSpeedReport + rating logic run.
vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { WebVitalsDetailModal } from "./WebVitalsDetailModal";

const REPORT = {
  url: "https://example.com",
  strategy: "mobile",
  performanceScore: 92,
  lcpMs: 1800,
  cls: 0.04,
  tbtMs: 120,
  fcpMs: 1200,
  ttfbMs: 300,
  siMs: 2600,
  opportunities: [
    {
      id: "uses-responsive-images",
      title: "Properly size images",
      description: "",
      savingsMs: 800,
    },
  ],
  fieldLcpMs: 2100,
  fieldCls: 0.05,
  fieldInpMs: 150,
  fieldSource: "url",
};

describe("WebVitalsDetailModal", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("fetches mobile PageSpeed on open and renders score, metrics, and opportunities", async () => {
    invokeMock.mockResolvedValue(REPORT);
    render(
      <WebVitalsDetailModal url="https://example.com" hostname="example.com" onClose={vi.fn()} />,
    );

    expect(await screen.findByText("92")).toBeInTheDocument();
    expect(screen.getByText("Properly size images")).toBeInTheDocument();
    // LCP appears in both the lab and the field grids.
    expect(screen.getAllByText("LCP").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("Largest content load").length).toBeGreaterThanOrEqual(2);
    expect(invokeMock).toHaveBeenCalledWith("get_pagespeed_report", {
      url: "https://example.com",
      strategy: "mobile",
    });
  });

  it("clarifies that the Lighthouse score is not the SiteCMD score (D9)", async () => {
    invokeMock.mockResolvedValue(REPORT);
    render(
      <WebVitalsDetailModal url="https://example.com" hostname="example.com" onClose={vi.fn()} />,
    );

    await screen.findByText("92");
    expect(screen.getByText(/not your SiteCMD score/i)).toBeInTheDocument();
    expect(
      screen.getByText(/two numbers can differ without either being wrong/i),
    ).toBeInTheDocument();
  });

  it("refetches with the desktop strategy when toggled", async () => {
    invokeMock.mockResolvedValue(REPORT);
    render(
      <WebVitalsDetailModal url="https://example.com" hostname="example.com" onClose={vi.fn()} />,
    );
    await screen.findByText("92");

    fireEvent.click(screen.getByRole("button", { name: /Desktop/i }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_pagespeed_report", {
        url: "https://example.com",
        strategy: "desktop",
      }),
    );
  });

  it("surfaces a PageSpeed error with a retry affordance", async () => {
    invokeMock.mockImplementation((cmd: string) =>
      cmd === "pagespeed_api_key_is_set"
        ? Promise.resolve(false)
        : Promise.reject("Service unavailable"),
    );
    render(
      <WebVitalsDetailModal url="https://example.com" hostname="example.com" onClose={vi.fn()} />,
    );

    expect(await screen.findByText(/Couldn't load PageSpeed/i)).toBeInTheDocument();
    // The raw rejection has no closing punctuation; userFacingError renders it as a
    // full sentence rather than showing the backend text verbatim.
    expect(screen.getByText("Service unavailable.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Try again/i })).toBeInTheDocument();
    // Non-rate-limit errors do not show the key prompt.
    expect(screen.queryByLabelText("PageSpeed API key")).not.toBeInTheDocument();
  });

  it("offers an inline API key on a 429 and retries after saving it", async () => {
    let psiCalls = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "pagespeed_api_key_is_set") return Promise.resolve(false);
      if (cmd === "set_pagespeed_api_key") return Promise.resolve(undefined);
      psiCalls += 1;
      return psiCalls === 1
        ? Promise.reject("PageSpeed API returned 429 Too Many Requests: rate limit exhausted")
        : Promise.resolve(REPORT);
    });
    render(
      <WebVitalsDetailModal url="https://example.com" hostname="example.com" onClose={vi.fn()} />,
    );

    const input = await screen.findByLabelText("PageSpeed API key");
    fireEvent.change(input, { target: { value: "  my-key  " } });
    fireEvent.click(screen.getByRole("button", { name: /Save & retry/i }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_pagespeed_api_key", { key: "my-key" }),
    );
    // Retry succeeds and renders the report.
    expect(await screen.findByText("92")).toBeInTheDocument();
  });
});
