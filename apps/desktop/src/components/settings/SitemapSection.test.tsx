import { render, screen } from "@testing-library/react";
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

  it("still asks for a project and environment when no URL is available", () => {
    renderSitemap(<SitemapSection />);

    expect(
      screen.getByText("Select a project and environment to manage sitemap pages."),
    ).toBeInTheDocument();
  });
});
