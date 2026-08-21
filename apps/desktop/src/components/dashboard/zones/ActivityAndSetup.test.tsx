import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RecentActivityCard, SetupCard } from "./ActivityAndSetup";
import type { ActivityRow, SetupRow } from "@/lib/dashboard/types";

const makeActivity = (overrides: Partial<ActivityRow> = {}): ActivityRow => ({
  id: "act-1",
  label: "Web scan",
  value: "12 issues · 87 score",
  valueColor: "amber",
  occurredAt: "2026-04-20T16:00:00Z",
  timeAgo: "14m ago",
  onOpen: vi.fn(),
  ...overrides,
});

const makeSetupRow = (overrides: Partial<SetupRow> = {}): SetupRow => ({
  id: "bootstrap:analytics",
  label: "Analytics",
  value: "Connect traffic source (Plausible, GA4, Cloudflare)",
  onOpen: vi.fn(),
  ...overrides,
});

describe("RecentActivityCard", () => {
  it("shows empty-state CTA when activity is empty", () => {
    const onOpenEmptyActivity = vi.fn();
    render(
      <RecentActivityCard
        activity={[]}
        onOpenEmptyActivity={onOpenEmptyActivity}
        onOpenAllActivity={vi.fn()}
      />,
    );
    expect(screen.getByText("Recent Activity")).toBeInTheDocument();
    expect(screen.getByText(/No recent activity yet/i)).toBeInTheDocument();
    fireEvent.click(screen.getByText(/Run your first scan/i));
    expect(onOpenEmptyActivity).toHaveBeenCalled();
  });

  it("renders activity items and routes clicks", () => {
    const onOpen = vi.fn();
    render(
      <RecentActivityCard
        activity={[makeActivity({ label: "Deploy", value: "v2.14 passed", onOpen })]}
        onOpenEmptyActivity={vi.fn()}
        onOpenAllActivity={vi.fn()}
      />,
    );
    expect(screen.getByText("Deploy")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Deploy").closest("button")!);
    expect(onOpen).toHaveBeenCalled();
  });

  it("shows a View All Activity button when activity exists", () => {
    const onOpenAllActivity = vi.fn();
    render(
      <RecentActivityCard
        activity={[makeActivity()]}
        onOpenEmptyActivity={vi.fn()}
        onOpenAllActivity={onOpenAllActivity}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "View All Activity" }));
    expect(onOpenAllActivity).toHaveBeenCalled();
  });
});

describe("SetupCard", () => {
  it("renders setup rows under the Finish Setup title and routes clicks", () => {
    const onOpen = vi.fn();
    render(<SetupCard rows={[makeSetupRow({ onOpen })]} />);

    expect(screen.getByText("Finish Setup")).toBeInTheDocument();
    expect(screen.getByText("Analytics")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Analytics").closest("button")!);
    expect(onOpen).toHaveBeenCalled();
  });

  it("renders nothing once every setup task is done", () => {
    const { container } = render(<SetupCard rows={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("does not use the retired What's Next framing", () => {
    render(<SetupCard rows={[makeSetupRow()]} />);
    expect(screen.queryByText(/What's Next/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Nothing scheduled/i)).not.toBeInTheDocument();
  });
});
