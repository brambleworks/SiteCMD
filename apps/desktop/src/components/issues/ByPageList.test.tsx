import { fireEvent, render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ByPageList } from "./ByPageList";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

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
});
