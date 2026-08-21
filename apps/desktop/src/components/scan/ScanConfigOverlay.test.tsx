import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ScanConfigOverlay } from "./ScanConfigOverlay";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { queryKeys } from "@/lib/query/query-keys";

const { invokeMock, toastWarning } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastWarning: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: invokeMock,
}));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ warning: toastWarning }),
}));

const PAGES = [
  {
    id: 1,
    site_id: 7,
    url: "https://example.com/",
    path: "/",
    title: "Homepage",
    last_seen_at: "2026-04-20T00:00:00Z",
    source: "sitemap",
  },
  {
    id: 2,
    site_id: 7,
    url: "https://example.com/docs",
    path: "/docs",
    title: "Docs",
    last_seen_at: "2026-04-20T00:00:00Z",
    source: "sitemap",
  },
  {
    id: 3,
    site_id: 7,
    url: "https://example.com/pricing",
    path: "/pricing",
    title: "Pricing",
    last_seen_at: "2026-04-20T00:00:00Z",
    source: "sitemap",
  },
];

describe("ScanConfigOverlay", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastWarning.mockReset();

    invokeMock.mockImplementation((command: string) => {
      if (command === "get_site_pages") return Promise.resolve(PAGES);
      if (command === "get_scan_scope") return Promise.resolve([]);
      if (command === "set_scan_scope") {
        return Promise.resolve({ revision: 1, routes: ["/"] });
      }
      return Promise.resolve(null);
    });
  });

  it("resolves a shared URL inside the active project", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_or_create_site_id") return Promise.resolve(17);
      if (command === "get_site_pages") return Promise.resolve([]);
      if (command === "get_scan_scope") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(
      <ScanConfigOverlay
        projectId={42}
        siteUrl="https://example.com/"
        onStart={vi.fn()}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("Homepage");
    expect(invokeMock).toHaveBeenCalledWith("get_or_create_site_id", {
      projectId: 42,
      url: "https://example.com/",
    });
  });

  it("defaults discovered page selection to the homepage only", async () => {
    const onStart = vi.fn();

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("1 of 3 pages selected");
    // The checklist is the site's scan scope, and the copy says so: a
    // narrowed selection narrows what the schedule watches too.
    expect(screen.getByText(/scheduled scans cover it too/i)).toBeInTheDocument();

    const runScanButton = screen.getByRole("button", { name: "Run Scan" });
    expect(runScanButton.querySelector("svg")).toHaveAttribute("fill", "currentColor");
    expect(runScanButton.querySelector("svg")).toHaveAttribute("stroke-width", "0");

    fireEvent.click(runScanButton);

    // The run dispatches after the selection is recorded as the site's scan
    // scope, so the schedule watches what was just chosen.
    await waitFor(() =>
      expect(onStart).toHaveBeenCalledWith({
        urls: ["https://example.com/"],
        axeEnabled: false,
        inspectLocalDatabases: false,
        scanType: "full",
      }),
    );
  });

  it("requires an explicit per-run opt-in before local database inspection", async () => {
    const onStart = vi.fn();
    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        projectId={42}
        projectPath="/Users/tester/project"
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    const databaseSwitch = await screen.findByRole("switch", {
      name: "Inspect local database schemas",
    });
    expect(databaseSwitch).toHaveAttribute("aria-checked", "false");
    fireEvent.click(databaseSwitch);
    expect(databaseSwitch).toHaveAttribute("aria-checked", "true");
    fireEvent.click(screen.getByRole("button", { name: "Run Scan" }));

    await waitFor(() =>
      expect(onStart).toHaveBeenCalledWith(
        expect.objectContaining({ inspectLocalDatabases: true }),
      ),
    );
  });

  it("opens on the site's stored scan scope", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_site_pages") return Promise.resolve(PAGES);
      if (command === "get_scan_scope") return Promise.resolve(["/", "/pricing"]);
      if (command === "set_scan_scope") {
        return Promise.resolve({ revision: 2, routes: ["/", "/pricing"] });
      }
      return Promise.resolve(null);
    });

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={vi.fn()}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("2 of 3 pages selected");
  });

  it("lists a scoped route that no longer has a discovered page", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_site_pages") return Promise.resolve(PAGES);
      if (command === "get_scan_scope") return Promise.resolve(["/", "/retired"]);
      if (command === "set_scan_scope") {
        return Promise.resolve({ revision: 3, routes: ["/", "/retired"] });
      }
      return Promise.resolve(null);
    });

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={vi.fn()}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("2 of 4 pages selected");
    expect(screen.getByText("/retired")).toBeInTheDocument();
  });

  it("records the selection as the scan scope before dispatching the run", async () => {
    const onStart = vi.fn();
    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("1 of 3 pages selected");
    fireEvent.click(screen.getByText("Pricing"));
    fireEvent.click(screen.getByRole("button", { name: "Run Scan" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_scan_scope", {
        siteId: 7,
        siteUrl: "https://example.com/",
        routes: ["/", "/pricing"],
      }),
    );
    expect(invokeMock).toHaveBeenCalledWith("sync_connected_scan_scope", { siteId: 7 });
    expect(onStart).toHaveBeenCalled();
  });

  it("invalidates connected state after the background scope PUT succeeds", async () => {
    const client = createTestQueryClient();
    const remoteKey = queryKeys.settings.connectedRemoteState(42, "https://example.com");
    client.setQueryData(remoteKey, { scopeRevision: 1 });

    render(
      <ScanConfigOverlay
        projectId={42}
        siteUrl="https://example.com"
        siteId={7}
        onStart={vi.fn()}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient(client) },
    );

    fireEvent.click(await screen.findByRole("button", { name: "Run Scan" }));

    await waitFor(() => expect(client.getQueryState(remoteKey)?.isInvalidated).toBe(true));
  });

  it("keeps the local run available when connected scope delivery needs a retry", async () => {
    const onStart = vi.fn();
    let rejectConnectedSync: ((reason: Error) => void) | undefined;
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_site_pages") return Promise.resolve(PAGES);
      if (command === "get_scan_scope") return Promise.resolve([]);
      if (command === "set_scan_scope") {
        return Promise.resolve({ revision: 1, routes: ["/"] });
      }
      if (command === "sync_connected_scan_scope") {
        return new Promise((_, reject) => {
          rejectConnectedSync = reject;
        });
      }
      return Promise.resolve(null);
    });

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    fireEvent.click(await screen.findByRole("button", { name: "Run Scan" }));

    await waitFor(() => expect(onStart).toHaveBeenCalled());
    expect(toastWarning).not.toHaveBeenCalled();

    rejectConnectedSync?.(new Error("connected service unavailable"));
    await waitFor(() =>
      expect(toastWarning).toHaveBeenCalledWith(
        "Local scope saved; connected scope still needs sync",
        "connected service unavailable",
      ),
    );
  });

  it("dispatches the canonical scope returned by the atomic scope write", async () => {
    const onStart = vi.fn();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_site_pages") return Promise.resolve(PAGES);
      if (command === "get_scan_scope") return Promise.resolve([]);
      if (command === "set_scan_scope") {
        return Promise.resolve({ revision: 2, routes: ["/", "/guides"] });
      }
      return Promise.resolve(null);
    });

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("1 of 3 pages selected");
    fireEvent.click(screen.getByText("Pricing"));
    fireEvent.click(screen.getByRole("button", { name: "Run Scan" }));

    await waitFor(() =>
      expect(onStart).toHaveBeenCalledWith({
        urls: ["https://example.com/", "https://example.com/guides"],
        axeEnabled: false,
        inspectLocalDatabases: false,
        scanType: "full",
      }),
    );
  });

  it("shows a refused scope write and holds the run", async () => {
    const onStart = vi.fn();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_site_pages") return Promise.resolve(PAGES);
      if (command === "get_scan_scope") return Promise.resolve([]);
      if (command === "set_scan_scope") {
        return Promise.reject(
          new Error("A scan scope holds at most 5000 routes; this one has 5001."),
        );
      }
      return Promise.resolve(null);
    });

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("1 of 3 pages selected");
    fireEvent.click(screen.getByRole("button", { name: "Run Scan" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("at most 5000 routes");
    expect(onStart).not.toHaveBeenCalled();
  });

  it("does not render a scan-mode picker", async () => {
    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={vi.fn()}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("1 of 3 pages selected");
    expect(screen.queryByRole("button", { name: /scan options/i })).not.toBeInTheDocument();
    expect(screen.queryByText("Full Scan")).not.toBeInTheDocument();
    expect(screen.queryByText("Full Web Scan")).not.toBeInTheDocument();
    expect(screen.queryByText("Web only")).not.toBeInTheDocument();
    expect(screen.queryByText("Code only")).not.toBeInTheDocument();
  });

  it("dispatches a Full scan when a project folder is linked", async () => {
    const onStart = vi.fn();

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        projectPath="/Users/dev/project"
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("1 of 3 pages selected");

    fireEvent.click(screen.getByRole("button", { name: "Run Scan" }));

    await waitFor(() =>
      expect(onStart).toHaveBeenCalledWith({
        urls: ["https://example.com/"],
        axeEnabled: false,
        inspectLocalDatabases: false,
        scanType: "full",
      }),
    );
  });

  it("dispatches a Full scan and keeps axe off when no project folder is linked", async () => {
    const onStart = vi.fn();

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("1 of 3 pages selected");
    expect(screen.queryByText(/Accessibility deep scan/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Run Scan" }));

    await waitFor(() =>
      expect(onStart).toHaveBeenCalledWith({
        urls: ["https://example.com/"],
        axeEnabled: false,
        inspectLocalDatabases: false,
        scanType: "full",
      }),
    );
  });

  it("hints to link a project folder so Code Scan can run", async () => {
    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={vi.fn()}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("1 of 3 pages selected");
    expect(screen.getByText(/Link a local project folder/i)).toBeInTheDocument();
  });

  it("shows the homepage target instead of an empty-page warning before discovery", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_site_pages") return Promise.resolve([]);
      if (command === "get_scan_scope") return Promise.resolve([]);
      if (command === "set_scan_scope") {
        return Promise.resolve({ revision: 1, routes: ["/"] });
      }
      return Promise.resolve(null);
    });
    const onStart = vi.fn();

    render(
      <ScanConfigOverlay
        siteUrl="http://localhost:4321/"
        siteId={11}
        onStart={onStart}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    await screen.findByText("Homepage");
    expect(screen.getAllByText("http://localhost:4321/").length).toBeGreaterThanOrEqual(1);
    expect(screen.queryByText(/No pages discovered yet/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Run Scan" }));

    await waitFor(() =>
      expect(onStart).toHaveBeenCalledWith({
        urls: ["http://localhost:4321/"],
        axeEnabled: false,
        inspectLocalDatabases: false,
        scanType: "full",
      }),
    );
  });

  it("publishes discovered pages to the shared sitemap cache", async () => {
    const client = createTestQueryClient();
    let discovered = false;
    invokeMock.mockImplementation((command: string) => {
      // The Discover control only renders while the site has no pages yet.
      if (command === "get_site_pages") return Promise.resolve(discovered ? PAGES : []);
      if (command === "get_scan_scope") return Promise.resolve([]);
      if (command === "set_scan_scope") {
        return Promise.resolve({ revision: 1, routes: ["/"] });
      }
      if (command === "refresh_sitemap") {
        discovered = true;
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });

    render(
      <ScanConfigOverlay
        siteUrl="https://example.com/"
        siteId={7}
        onStart={vi.fn()}
        onCancel={vi.fn()}
      />,
      { wrapper: withQueryClient(client) },
    );

    const discoverButton = await screen.findByRole("button", { name: /discover pages/i });
    expect(client.getQueryData(queryKeys.settings.sitemapPages(7))).toHaveLength(0);

    fireEvent.click(discoverButton);

    await screen.findByText("1 of 3 pages selected");
    // The shared entry, not just this overlay's local state, has to carry the
    // refreshed list - that is what Settings > Site Setup renders from.
    expect(client.getQueryData(queryKeys.settings.sitemapPages(7))).toHaveLength(3);
  });
});
