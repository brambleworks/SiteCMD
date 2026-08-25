import React from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, hasFeatureMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  hasFeatureMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    tier: "free",
    licenseInfo: {
      tier: "free",
      status: "none",
      plan_name: "Free",
      is_active: false,
      expires_at: null,
      checkout_urls: {
        core_monthly: "",
        core_annual: "",
        pro_monthly: "",
        pro_annual: "",
      },
      customer_portal_url: "",
    },
    isLoading: false,
    hasFeature: hasFeatureMock,
    activateLicense: vi.fn(),
    deactivateLicense: vi.fn(),
    refreshLicense: vi.fn(),
  }),
}));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  }),
}));
vi.mock("@/components/icons/ServiceIcon", () => ({
  ServiceIcon: () => React.createElement("span", null, "icon"),
}));
vi.mock("@/components/settings/IntegrationDataViews", () => ({
  PlausibleDataView: () => null,
  CloudflareDataView: () => null,
  UptimeRobotDataView: () => null,
  GenericIntegrationDataView: () => null,
}));
vi.mock("@/components/ui/external-link", () => ({
  ExtLink: ({
    children,
    href,
    className,
  }: {
    children: React.ReactNode;
    href: string;
    className?: string;
  }) => React.createElement("a", { href, className }, children),
}));

import { IntegrationSettings } from "./IntegrationSettings";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

function renderIntegrationSettings(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient(createTestQueryClient()) });
}

/** Find the full-width row whose title matches, returning the row <button>. */
function rowByName(name: string): HTMLElement {
  const button = screen.getByText(name).closest("button");
  if (!button) throw new Error(`No integration row for "${name}"`);
  return button;
}

describe("IntegrationSettings", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    hasFeatureMock.mockReset();
    hasFeatureMock.mockImplementation((feature: string) => feature === "integration_connect");
  });

  it("auto-fetches data and groups connected integrations under Active", async () => {
    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "get_integrations":
          return [
            {
              integrationType: "plausible",
              apiKey: null,
              siteId: "example.com",
              extra: null,
              enabled: true,
            },
            {
              integrationType: "github",
              apiKey: null,
              siteId: "acme/sitecmd",
              extra: null,
              enabled: true,
            },
          ];
        case "fetch_integration_data":
          return {
            integrationType: args?.integrationType,
            data: {},
            fetchedAt: "2026-04-13T12:00:00Z",
            error: null,
          };
        default:
          return null;
      }
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://example.com" />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_integrations", { projectId: 7 });
    });

    await waitFor(() => {
      const fetchCalls = invokeMock.mock.calls.filter(
        ([command]) => command === "fetch_integration_data",
      );
      expect(fetchCalls).toHaveLength(2);
      const fetchedTypes = fetchCalls
        .map(([, args]) => (args as Record<string, unknown>).integrationType)
        .sort();
      expect(fetchedTypes).toEqual(["github", "plausible"]);
    });

    // Both connected: an "Active" section with a Manage action and a connected dot.
    expect(screen.getByText("Active")).toBeInTheDocument();
    const plausibleRow = rowByName("Plausible Analytics");
    const githubRow = rowByName("GitHub");
    expect(plausibleRow).toHaveTextContent("Manage");
    expect(githubRow).toHaveTextContent("Manage");
    expect(within(plausibleRow).getByLabelText("Connected")).toBeInTheDocument();
    expect(within(githubRow).getByLabelText("Connected")).toBeInTheDocument();
  });

  it("keeps configured integrations connected while live data is still loading", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_integrations":
          return [
            {
              integrationType: "plausible",
              apiKey: null,
              siteId: "example.com",
              extra: null,
              enabled: true,
            },
          ];
        case "fetch_integration_data":
          return new Promise(() => {});
        default:
          return null;
      }
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://example.com" />,
    );

    await waitFor(() => {
      expect(rowByName("Plausible Analytics")).toHaveTextContent("Manage");
    });
    const plausibleRow = rowByName("Plausible Analytics");
    expect(plausibleRow).toHaveTextContent("Manage");
    expect(within(plausibleRow).getByLabelText("Connected")).toBeInTheDocument();
  });

  it("opens setup in a modal from a row when nothing is configured", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_integrations") {
        return [];
      }
      return null;
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://sitecmd.test" />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_integrations", { projectId: 7 });
    });

    expect(screen.getAllByText("Deploys & CI").length).toBeGreaterThan(0);

    // Nothing is configured: no Active section, every row offers "Set up".
    expect(screen.queryByText("Active")).not.toBeInTheDocument();
    const plausibleRow = rowByName("Plausible Analytics");
    expect(plausibleRow).toHaveTextContent("Set up");
    expect(within(plausibleRow).queryByLabelText("Connected")).toBeNull();

    // The form lives in a modal, not inline in the row.
    expect(screen.queryByText("Open Plausible API Keys →")).not.toBeInTheDocument();
    fireEvent.click(plausibleRow);
    expect(screen.getByRole("dialog", { name: "Plausible Analytics" })).toBeInTheDocument();
    expect(screen.getByText("Open Plausible API Keys →")).toBeInTheDocument();

    // The other services still render as rows behind the modal.
    expect(rowByName("Cloudflare")).toBeInTheDocument();
    expect(rowByName("GitHub")).toBeInTheDocument();
  });

  it("offers Manage with no connected dot for a configured integration with a provider error", async () => {
    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "get_integrations":
          return [
            {
              integrationType: "plausible",
              apiKey: null,
              siteId: "wrong-site.example",
              extra: null,
              enabled: true,
            },
          ];
        case "fetch_integration_data":
          return {
            integrationType: args?.integrationType,
            data: {},
            fetchedAt: "2026-04-13T12:00:00Z",
            error: "Plausible API returned 404 Not Found",
          };
        default:
          return null;
      }
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://sitecmd.test" />,
    );

    await screen.findByText("Plausible Analytics");
    // Once the error resolves the row drops the connected dot and leaves Active.
    await waitFor(() => {
      expect(within(rowByName("Plausible Analytics")).queryByLabelText("Connected")).toBeNull();
    });
    expect(rowByName("Plausible Analytics")).toHaveTextContent("Manage");
    expect(screen.queryByText("Plausible API returned 404 Not Found")).not.toBeInTheDocument();

    fireEvent.click(rowByName("Plausible Analytics"));

    expect(screen.getByText("Save credentials")).toBeInTheDocument();
    expect(
      screen.getByText("Last check: Plausible API returned 404 Not Found"),
    ).toBeInTheDocument();
  });

  it("lets users repair missing connected integration credentials from the modal", async () => {
    let credentialsSaved = false;
    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "get_integrations":
          return [
            {
              integrationType: "plausible",
              apiKey: null,
              siteId: "example.com",
              extra: null,
              enabled: true,
            },
          ];
        case "fetch_integration_data":
          return {
            integrationType: args?.integrationType,
            data: {},
            fetchedAt: "2026-04-13T12:00:00Z",
            error: credentialsSaved ? null : "No Plausible API key configured",
          };
        case "save_integration":
          credentialsSaved = true;
          return null;
        default:
          return null;
      }
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://example.com" />,
    );

    await screen.findByText("Plausible Analytics");
    await waitFor(() => {
      expect(within(rowByName("Plausible Analytics")).queryByLabelText("Connected")).toBeNull();
    });

    fireEvent.click(rowByName("Plausible Analytics"));
    fireEvent.change(screen.getByPlaceholderText("Paste api key"), {
      target: { value: "plausible-token" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save credentials" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("save_integration", {
        projectId: 7,
        config: {
          integrationType: "plausible",
          apiKey: "plausible-token",
          siteId: "example.com",
          extra: null,
          enabled: true,
        },
      });
    });
    await waitFor(() => {
      const row = rowByName("Plausible Analytics");
      expect(within(row).getByLabelText("Connected")).toBeInTheDocument();
      const fetchCalls = invokeMock.mock.calls.filter(
        ([command]) => command === "fetch_integration_data",
      );
      expect(fetchCalls).toHaveLength(2);
    });
  });

  it("finishes Search Console setup automatically when Google returns one available site", async () => {
    let connected = false;
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_integrations":
          return connected
            ? [
                {
                  integrationType: "googlesearchconsole",
                  apiKey: null,
                  siteId: "https://example.com/",
                  extra: null,
                  enabled: true,
                },
              ]
            : [];
        case "connect_google":
          return { flow_id: "google-flow" };
        case "complete_google_oauth":
          return {
            ga4_properties: [],
            gsc_sites: [{ site_url: "https://example.com/", permission: "siteOwner" }],
          };
        case "save_google_integration":
          connected = true;
          return "Connected";
        case "fetch_integration_data":
          return {
            integrationType: "googlesearchconsole",
            data: {},
            fetchedAt: "2026-04-13T12:00:00Z",
            error: null,
          };
        default:
          return null;
      }
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://sitecmd.test" />,
    );

    fireEvent.click(await screen.findByText("Google Search Console"));
    fireEvent.click(screen.getByRole("button", { name: "Sign in with Google" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("save_google_integration", {
        projectId: 7,
        flowId: "google-flow",
        integrationType: "googlesearchconsole",
        siteId: "https://example.com/",
      });
    });
  });

  it("keeps a visible Search Console setup error when Google authorization cannot be completed", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_integrations":
          return [];
        case "connect_google":
          return { flow_id: "google-flow" };
        case "complete_google_oauth":
          throw new Error("Token exchange returned error");
        default:
          return null;
      }
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://sitecmd.test" />,
    );

    fireEvent.click(await screen.findByText("Google Search Console"));
    fireEvent.click(screen.getByRole("button", { name: "Sign in with Google" }));

    await waitFor(() => {
      expect(
        screen.getByText(
          "Google returned the browser authorization, but SiteCMD could not finish setup.",
        ),
      ).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: "Try setup again" })).toBeInTheDocument();
  });

  it("shows the Search Console site picker modal when Google returns multiple sites", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_integrations":
          return [];
        case "connect_google":
          return { flow_id: "google-flow" };
        case "complete_google_oauth":
          return {
            ga4_properties: [],
            gsc_sites: [
              { site_url: "https://example.com/", permission: "siteOwner" },
              { site_url: "https://other.test/", permission: "siteOwner" },
            ],
          };
        default:
          return null;
      }
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://sitecmd.test" />,
    );

    fireEvent.click(await screen.findByText("Google Search Console"));
    fireEvent.click(screen.getByRole("button", { name: "Sign in with Google" }));

    await waitFor(() => {
      expect(screen.getByText("Choose a Search Console site")).toBeInTheDocument();
    });
    // The connect modal closed; only the picker dialog remains. The picker
    // opens via an effect after its content renders, so wait for exactly one
    // open dialog instead of sampling the gap between close and showModal.
    await waitFor(() => expect(screen.getAllByRole("dialog")).toHaveLength(1));
    expect(screen.getByRole("option", { name: "https://example.com/" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "https://other.test/" })).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("save_google_integration", expect.anything());
  });

  it("connects both Google Analytics and Search Console from one OAuth grant", async () => {
    const savedIntegrations: Array<{ integrationType: string; siteId: string }> = [];
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_integrations":
          return savedIntegrations.map((s) => ({
            integrationType: s.integrationType,
            apiKey: null,
            siteId: s.siteId,
            extra: null,
            enabled: true,
          }));
        case "connect_google":
          return { flow_id: "dual-flow" };
        case "complete_google_oauth":
          return {
            ga4_properties: [
              {
                property_id: "properties/111",
                display_name: "My Site GA4",
                account_name: "My Account",
              },
            ],
            gsc_sites: [{ site_url: "https://example.com/", permission: "siteOwner" }],
          };
        case "save_google_integration":
          savedIntegrations.push({ integrationType: "googlesearchconsole", siteId: "x" });
          return "Connected";
        case "fetch_integration_data":
          return {
            integrationType: "googlesearchconsole",
            data: {},
            fetchedAt: new Date().toISOString(),
            error: null,
          };
        default:
          return null;
      }
    });

    renderIntegrationSettings(
      <IntegrationSettings projectId={7} projectName="SiteCMD" url="https://example.com" />,
    );

    // Open the Google Analytics row and start the grant.
    fireEvent.click(await screen.findByText("Google Analytics (GA4)"));
    fireEvent.click(screen.getByRole("button", { name: "Sign in with Google" }));

    await waitFor(() => {
      expect(screen.getByText("Choose what to connect")).toBeInTheDocument();
    });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByLabelText("Google Analytics property")).toBeInTheDocument();
    expect(screen.getByLabelText("Search Console site")).toBeInTheDocument();
    expect(screen.getAllByRole("option", { name: "Do not connect" }).length).toBeGreaterThanOrEqual(
      1,
    );

    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => {
      const saveCalls = invokeMock.mock.calls.filter(([cmd]) => cmd === "save_google_integration");
      expect(saveCalls).toHaveLength(2);
      const types = saveCalls
        .map(([, args]) => (args as Record<string, unknown>).integrationType)
        .sort();
      expect(types).toEqual(["googleanalytics", "googlesearchconsole"]);
      const flowIds = saveCalls.map(([, args]) => (args as Record<string, unknown>).flowId);
      expect(flowIds[0]).toBe("dual-flow");
      expect(flowIds[1]).toBe("dual-flow");
    });
  });
});
