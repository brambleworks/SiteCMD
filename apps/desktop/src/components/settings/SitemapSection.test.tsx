import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { SitemapSection } from "./SitemapSection";
import { withQueryClient } from "@/test-utils/query-client";

function renderSitemap(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

describe("SitemapSection", () => {
  it("uses the selected environment URL while the sitemap site ID resolves", () => {
    invokeMock.mockResolvedValue(42);

    renderSitemap(<SitemapSection siteUrl="https://example.com" />);

    expect(
      screen.queryByText("Select a project and environment to manage sitemap pages."),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("status", { name: "Sitemap settings loading state" })).toBeVisible();
  });

  it("bounds the reviewed page list and reveals the rest through the pager", async () => {
    const sitePages = Array.from({ length: 3000 }, (_, index) => ({
      id: index + 1,
      site_id: 5,
      url: `https://example.com/page-${index + 1}`,
      path: `/page-${index + 1}`,
      title: null,
      last_seen_at: "2026-04-20T00:00:00Z",
      source: "sitemap",
    }));
    invokeMock.mockImplementation((command: string) =>
      command === "get_site_pages" ? Promise.resolve(sitePages) : Promise.resolve(5),
    );

    const { container } = renderSitemap(
      <SitemapSection siteUrl="https://example.com" siteId={5} />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Review Pages" }));

    // A sitemap with thousands of pages mounts one page of rows, not all of them.
    expect(container.querySelectorAll(".settings-page-row")).toHaveLength(50);
    expect(screen.getByText("/page-1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next sitemap page" }));

    expect(container.querySelectorAll(".settings-page-row")).toHaveLength(50);
    expect(screen.getByText("/page-51")).toBeInTheDocument();
    expect(screen.queryByText("/page-1")).not.toBeInTheDocument();

    // Searching narrows the list and reopens it on the first page.
    fireEvent.change(screen.getByPlaceholderText("Search pages…"), {
      target: { value: "/page-2000" },
    });
    expect(container.querySelectorAll(".settings-page-row")).toHaveLength(1);
    expect(screen.getByText("/page-2000")).toBeInTheDocument();
  });

  it("still asks for a project and environment when no URL is available", () => {
    renderSitemap(<SitemapSection />);

    expect(
      screen.getByText("Select a project and environment to manage sitemap pages."),
    ).toBeInTheDocument();
  });
});
