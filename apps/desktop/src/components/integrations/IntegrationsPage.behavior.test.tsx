import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@/components/settings/IntegrationSettings", () => ({
  IntegrationSettings: ({ projectName }: { projectName: string }) =>
    React.createElement("div", null, `IntegrationSettings:${projectName}`),
}));

import { IntegrationsPage } from "./IntegrationsPage";
import { withQueryClient } from "@/test-utils/query-client";

function renderIntegrations(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

describe("IntegrationsPage behavior", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("shows the real retry state when integrations cannot load", async () => {
    invokeMock.mockRejectedValue(new Error("offline"));

    renderIntegrations(
      <IntegrationsPage projectId={7} projectName="Example Site" url="https://example.com" />,
    );

    await waitFor(() => {
      expect(screen.getByText("Integrations could not load")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(2);
    });
  });

  it("shows a page-shaped loading skeleton while integrations are loading", () => {
    invokeMock.mockImplementation(() => new Promise(() => {}));

    renderIntegrations(
      <IntegrationsPage projectId={7} projectName="Example Site" url="https://example.com" />,
    );

    expect(screen.getByLabelText("Integrations loading state")).toBeInTheDocument();
  });

  it("renders the management grid without a duplicate page heading", async () => {
    invokeMock.mockResolvedValue([
      { integration_type: "plausible", enabled: true },
      { integration_type: "cloudflare", enabled: true },
      { integration_type: "github", enabled: true },
    ]);

    renderIntegrations(
      <IntegrationsPage projectId={7} projectName="Example Site" url="https://example.com" />,
    );

    await waitFor(() => {
      expect(screen.getByText("IntegrationSettings:Example Site")).toBeInTheDocument();
    });

    expect(screen.queryByText("Connect the tools you already use")).not.toBeInTheDocument();
    expect(
      screen.queryByText("Active integrations first. Available ones below."),
    ).not.toBeInTheDocument();
  });

  it("scrolls to the focused integration card after configs load", async () => {
    const originalScroll = (Element.prototype as unknown as { scrollIntoView?: () => void })
      .scrollIntoView;
    const scrollSpy = vi.fn();
    Element.prototype.scrollIntoView = scrollSpy;
    const onFocusHandled = vi.fn();

    const target = document.createElement("div");
    target.setAttribute("data-integration", "plausible");
    document.body.appendChild(target);

    try {
      invokeMock.mockResolvedValue([{ integration_type: "plausible", enabled: true }]);

      renderIntegrations(
        <IntegrationsPage
          projectId={1}
          projectName="Test"
          url="https://example.com"
          focusIntegration="plausible"
          onFocusHandled={onFocusHandled}
        />,
      );

      await screen.findByText("IntegrationSettings:Test");
      await waitFor(() => {
        expect(scrollSpy).toHaveBeenCalled();
        expect(onFocusHandled).toHaveBeenCalled();
      });
    } finally {
      document.body.removeChild(target);
      if (originalScroll === undefined) {
        delete (Element.prototype as unknown as { scrollIntoView?: () => void }).scrollIntoView;
      } else {
        Element.prototype.scrollIntoView = originalScroll;
      }
    }
  });
});
