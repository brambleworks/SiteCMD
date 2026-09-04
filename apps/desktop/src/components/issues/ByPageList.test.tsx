import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ByPageList } from "./ByPageList";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { queryKeys } from "@/lib/query/query-keys";
import type { PageSummary } from "@/lib/types";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function makePages(count: number, host = "example.com"): PageSummary[] {
  return Array.from({ length: count }, (_, index) => ({
    pageUrl: `https://${host}/page-${index + 1}`,
    label: `Page ${index + 1}`,
    issueCount: 1,
    maxSeverity: "high",
    sources: ["web_scan"],
  }));
}

describe("ByPageList", () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue([
      {
        pageUrl: "https://example.com/pricing",
        label: "https://example.com/pricing",
        issueCount: 4,
        maxSeverity: "high",
        sources: ["web_scan"],
      },
      {
        pageUrl: "__project_wide__",
        label: "Project-wide",
        issueCount: 2,
        maxSeverity: "critical",
        sources: ["updates"],
      },
    ]);
  });

  it("shows readable page rows and excludes synthetic project-wide findings", async () => {
    const onSelectPage = vi.fn();
    render(<ByPageList projectId={1} envUrl="https://example.com" onSelectPage={onSelectPage} />, {
      wrapper: withQueryClient(),
    });

    const pageButton = await screen.findByRole("button", {
      name: "Open /pricing on example.com: 4 open issues",
    });
    expect(screen.getByText("1 affected page")).toBeInTheDocument();
    expect(screen.getByText("Highest: High")).toBeInTheDocument();
    expect(screen.getByText("4 open issues")).toBeInTheDocument();
    expect(screen.queryByText("Project-wide")).not.toBeInTheDocument();
    expect(screen.queryByText("web_scan")).not.toBeInTheDocument();
    expect(screen.queryByText("code_scan")).not.toBeInTheDocument();
    expect(screen.queryByRole("navigation")).not.toBeInTheDocument();

    fireEvent.click(pageButton);
    expect(onSelectPage).toHaveBeenCalledWith("https://example.com/pricing");
  });

  it("shows a retryable error instead of an empty page list when the read fails", async () => {
    invokeMock.mockRejectedValue(new Error("database unavailable"));

    render(<ByPageList projectId={1} envUrl="https://example.com" onSelectPage={() => {}} />, {
      wrapper: withQueryClient(),
    });

    expect(await screen.findByText("Affected pages could not load")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });

  it("reuses the page index cache when the view remounts", async () => {
    const queryClient = createTestQueryClient();
    const wrapper = withQueryClient(queryClient);
    const props = {
      projectId: 1,
      envUrl: "https://example.com",
      onSelectPage: vi.fn(),
    };
    const first = render(<ByPageList {...props} />, { wrapper });
    await screen.findByRole("button", { name: /open \/pricing/i });
    first.unmount();

    render(<ByPageList {...props} />, { wrapper });

    expect(await screen.findByRole("button", { name: /open \/pricing/i })).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("renders 50 of 5000 affected pages and keeps the final page reachable", () => {
    const pages = makePages(5000);
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(queryKeys.issuePages.forEnv(1, "https://example.com"), [
      { ...pages[0], pageUrl: "__project_wide__" },
      { ...pages[0], pageUrl: " " },
      ...pages,
    ]);
    const onSelectPage = vi.fn();
    const { container } = render(
      <ByPageList projectId={1} envUrl="https://example.com" onSelectPage={onSelectPage} />,
      { wrapper: withQueryClient(queryClient) },
    );

    expect(screen.getByText("5000 affected pages")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /^Open \/page-/ })).toHaveLength(50);
    expect(container.querySelectorAll("*").length).toBeLessThan(1000);
    expect(screen.getByRole("status")).toHaveTextContent("Showing 1-50 of 5000 affected pages");
    expect(screen.queryByRole("button", { name: /Open \/page-51 on/ })).not.toBeInTheDocument();

    const navigation = screen.getByRole("navigation", { name: "Affected page results" });
    fireEvent.click(within(navigation).getByRole("button", { name: "Page 100" }));

    expect(screen.getAllByRole("button", { name: /^Open \/page-/ })).toHaveLength(50);
    expect(screen.getByRole("status")).toHaveTextContent(
      "Showing 4951-5000 of 5000 affected pages",
    );
    expect(within(navigation).getByRole("button", { name: "Page 100" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(within(navigation).queryByRole("button", { name: "Next results page" })).toBeNull();
    fireEvent.click(
      screen.getByRole("button", { name: "Open /page-5000 on example.com: 1 open issue" }),
    );
    expect(onSelectPage).toHaveBeenCalledWith("https://example.com/page-5000");

    fireEvent.click(within(navigation).getByRole("button", { name: "Previous results page" }));
    expect(screen.getByRole("status")).toHaveTextContent(
      "Showing 4901-4950 of 5000 affected pages",
    );
  });

  it("supports keyboard paging and announces the displayed range", async () => {
    const user = userEvent.setup();
    invokeMock.mockResolvedValue(makePages(125));
    render(<ByPageList projectId={1} envUrl="https://example.com" onSelectPage={vi.fn()} />, {
      wrapper: withQueryClient(),
    });
    const navigation = await screen.findByRole("navigation", { name: "Affected page results" });
    const next = within(navigation).getByRole("button", { name: "Next results page" });
    next.focus();
    await user.keyboard("{Enter}");

    expect(screen.getByRole("status")).toHaveTextContent("Showing 51-100 of 125 affected pages");
    expect(within(navigation).getByRole("button", { name: "Page 2" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(within(navigation).getByRole("button", { name: "Page 1" })).not.toHaveAttribute(
      "aria-current",
    );
    expect(next).toHaveFocus();
  });

  it("clamps the selected page when a refreshed result becomes shorter", async () => {
    const queryClient = createTestQueryClient();
    const key = queryKeys.issuePages.forEnv(1, "https://example.com");
    queryClient.setQueryData(key, makePages(150));
    render(<ByPageList projectId={1} envUrl="https://example.com" onSelectPage={vi.fn()} />, {
      wrapper: withQueryClient(queryClient),
    });
    fireEvent.click(screen.getByRole("button", { name: "Page 3" }));

    act(() => queryClient.setQueryData(key, makePages(75)));

    await waitFor(() =>
      expect(screen.getAllByRole("button", { name: /^Open \/page-/ })).toHaveLength(25),
    );
    expect(screen.getByRole("status")).toHaveTextContent("Showing 51-75 of 75 affected pages");
    expect(screen.getByRole("button", { name: "Page 2" })).toHaveAttribute("aria-current", "page");
  });

  it.each([
    { projectId: 2, envUrl: "https://example.com" },
    { projectId: 1, envUrl: "https://staging.example.com" },
  ])("resets paging when the target changes to $projectId / $envUrl", (target) => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(queryKeys.issuePages.forEnv(1, "https://example.com"), makePages(125));
    queryClient.setQueryData(
      queryKeys.issuePages.forEnv(target.projectId, target.envUrl),
      makePages(125),
    );
    const onSelectPage = vi.fn();
    const { rerender } = render(
      <ByPageList projectId={1} envUrl="https://example.com" onSelectPage={onSelectPage} />,
      { wrapper: withQueryClient(queryClient) },
    );
    fireEvent.click(screen.getByRole("button", { name: "Page 3" }));
    rerender(<ByPageList {...target} onSelectPage={onSelectPage} />);

    expect(screen.getByRole("status")).toHaveTextContent("Showing 1-50 of 125 affected pages");
    expect(screen.getByRole("button", { name: "Page 1" })).toHaveAttribute("aria-current", "page");
  });
});
