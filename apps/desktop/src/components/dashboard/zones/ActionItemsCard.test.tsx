import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CompactTrendModel } from "@/components/dashboard/compact-trend-model";
import { ActionItemsCard, type ActionItemsData } from "./ActionItemsCard";

function trend(overrides: Partial<CompactTrendModel> = {}): CompactTrendModel {
  return {
    key: "issues-trend",
    label: "Issues trend",
    currentValue: "12",
    detail: "2 critical issues",
    deltaLabel: "-2 since last checked",
    tone: "improving",
    series: [14, 12],
    ...overrides,
  };
}

const baseItems: ActionItemsData = {
  cards: [
    {
      key: "issues",
      label: "Issues",
      value: "12 Open",
      detail: "2 Critical · 3 High · 4 Medium · 3 Low",
      trend: trend(),
      onClick: vi.fn(),
    },
    {
      key: "updates",
      label: "Updates",
      value: "5 Available",
      detail: "1 Security · 1 Major · 2 Minor · 1 Patch",
      trend: trend({ key: "updates-trend", label: "Updates trend" }),
      onClick: vi.fn(),
    },
  ],
};

describe("ActionItemsCard", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("renders Issues and Updates as two standalone cards", () => {
    render(<ActionItemsCard items={baseItems} />);

    expect(screen.getByText("Issues")).toBeInTheDocument();
    expect(screen.getByText("Updates")).toBeInTheDocument();
    // Two separate clickable cards, no "Action Items" wrapper.
    expect(screen.queryByText("Action Items")).not.toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(2);
  });

  it("routes primary card clicks", () => {
    const onIssuesClick = vi.fn();
    render(
      <ActionItemsCard
        items={{
          cards: [{ ...baseItems.cards[0], onClick: onIssuesClick }, ...baseItems.cards.slice(1)],
        }}
      />,
    );

    fireEvent.click(screen.getByText("Issues").closest("button")!);
    expect(onIssuesClick).toHaveBeenCalled();
  });

  it("renders nothing when there are no cards", () => {
    const { container } = render(<ActionItemsCard items={{ cards: [] }} />);
    expect(container).toBeEmptyDOMElement();
  });
});
