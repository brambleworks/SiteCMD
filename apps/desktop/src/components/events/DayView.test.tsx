import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DayView } from "./DayView";

describe("DayView", () => {
  it("shows compare pills and exact reopen actions for expanded update events", async () => {
    const onOpenTarget = vi.fn();

    render(
      <DayView
        date={new Date("2026-04-11T12:00:00Z")}
        events={[
          {
            id: 45,
            projectId: 7,
            eventType: "update",
            severity: "info",
            occurredAtMs: new Date("2026-04-11T12:30:00Z").getTime(),
            title: "Update verified: react",
            summary:
              "react 18.2.0 -> 19.0.0 cleared from Updates. Next up: react-dom 18.2.0 -> 19.0.0 • major.",
            detail: JSON.stringify({
              page: "updates",
              url: "https://example.com",
              item_id: "npm:react-dom",
              item_label: "react 18.2.0 -> 19.0.0 • major",
              verified_label: "react 18.2.0 -> 19.0.0 • major",
              next_item_label: "react-dom 18.2.0 -> 19.0.0 • major",
              status_before: "Pending",
              status_after: "Verified",
              remaining_updates: 1,
              workflow_label: "Exact package verified",
              reason: "dependency-verification",
            }),
            source: "internal",
            sourceId: "updates-verify:7:npm:react:verified:1",
            metadata: null,
            affectedCheckIds: null,
          },
        ]}
        workSummary={null}
        onOpenTarget={onOpenTarget}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Update verified: react/i }));

    expect(await screen.findByText("Pending -> Verified")).toBeInTheDocument();
    expect(screen.getByText("Next up: react-dom 18.2.0 -> 19.0.0 • major")).toBeInTheDocument();
    expect(screen.queryByText("status_before")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Open Package Update/i }));
    expect(onOpenTarget).toHaveBeenCalledWith({
      page: "updates",
      projectId: 7,
      url: "https://example.com",
      itemId: "npm:react-dom",
      reason: "dependency-verification",
    });
  });
});
