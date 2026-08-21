import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { IdentityHealthStrip } from "./IdentityHealthStrip";

const baseProps = {
  domain: "sitecmd.com",
  stack: { framework: "Next.js", host: "Vercel", environment: "prod" },
  sslDaysRemaining: 68,
  verdict: { kind: "attention" as const, phrase: "Attention needed", reasons: [] },
  lastScanIso: new Date(Date.now() - 14 * 60_000).toISOString(), // 14m ago
  unreadAlertCount: 0,
  onOpenAlerts: vi.fn(),
};

describe("IdentityHealthStrip", () => {
  it("renders domain without duplicating verdict text", () => {
    render(<IdentityHealthStrip {...baseProps} />);
    expect(screen.getByText("sitecmd.com")).toBeInTheDocument();
    expect(screen.queryByText("Attention needed")).not.toBeInTheDocument();
  });

  it("renders stack chip joining parts with middle-dot", () => {
    render(<IdentityHealthStrip {...baseProps} />);
    expect(screen.getByText("Next.js · Vercel · prod")).toBeInTheDocument();
  });

  it("renders SSL days remaining", () => {
    render(<IdentityHealthStrip {...baseProps} />);
    expect(screen.getByText("SSL certificate expires in 68 days")).toBeInTheDocument();
  });

  it("omits stack chip when all stack fields are null", () => {
    render(
      <IdentityHealthStrip
        {...baseProps}
        stack={{ framework: null, host: null, environment: null }}
      />,
    );
    // The stack chip joins parts with " · " - none of the framework/host/env names should appear
    expect(screen.queryByText(/Next\.js/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Vercel/)).not.toBeInTheDocument();
  });

  it("omits SSL when sslDaysRemaining is null", () => {
    render(<IdentityHealthStrip {...baseProps} sslDaysRemaining={null} />);
    expect(screen.queryByText(/SSL/)).not.toBeInTheDocument();
  });

  it("does not duplicate critical issue counts in the strip", () => {
    render(<IdentityHealthStrip {...baseProps} />);
    expect(screen.queryByText(/critical/)).not.toBeInTheDocument();
  });

  it("shows a clickable unread-alerts count that opens the Alerts page", () => {
    const onOpenAlerts = vi.fn();
    render(<IdentityHealthStrip {...baseProps} unreadAlertCount={3} onOpenAlerts={onOpenAlerts} />);

    const alertsButton = screen.getByRole("button", { name: "Open alerts: 3 alerts" });
    expect(alertsButton).toHaveTextContent("3 alerts");
    fireEvent.click(alertsButton);
    expect(onOpenAlerts).toHaveBeenCalledTimes(1);
  });

  it("uses the singular form for a single unread alert", () => {
    render(<IdentityHealthStrip {...baseProps} unreadAlertCount={1} />);
    expect(screen.getByRole("button", { name: "Open alerts: 1 alert" })).toHaveTextContent(
      "1 alert",
    );
  });

  it("hides the alerts count entirely when nothing is unread", () => {
    render(<IdentityHealthStrip {...baseProps} unreadAlertCount={0} />);
    expect(screen.queryByRole("button", { name: /Open alerts/ })).not.toBeInTheDocument();
    expect(screen.queryByText(/alert/)).not.toBeInTheDocument();
  });
});
