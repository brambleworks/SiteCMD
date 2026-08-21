import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SiteBaseline, SiteBaselineField } from "@/generated/ipc-bindings";
import { SiteBaselineCard } from "./SiteBaselineCard";

function field(overrides: Partial<SiteBaselineField> = {}): SiteBaselineField {
  return {
    field: "security_headers",
    label: "Security headers",
    status: "good",
    origin: "Recorded from the first scan that saw it",
    recordedAt: Date.now() - 60_000,
    goodLines: ["x-frame-options: DENY"],
    changedLines: [],
    changeDigest: "",
    canDismiss: true,
    changeFirstSeenAt: 0,
    ...overrides,
  };
}

function baseline(fields: SiteBaselineField[]): SiteBaseline {
  return { revision: 4, fields };
}

const changedField = field({
  status: "changed",
  changedLines: ["x-frame-options: SAMEORIGIN"],
  changeDigest: "abc123",
  changeFirstSeenAt: Date.now() - 30_000,
});

describe("SiteBaselineCard", () => {
  it("renders nothing until a scan has established a baseline", () => {
    const { container } = render(<SiteBaselineCard baseline={baseline([])} onDecide={vi.fn()} />);

    expect(container).toBeEmptyDOMElement();
  });

  it("offers no decision on a family that matches its baseline", () => {
    render(<SiteBaselineCard baseline={baseline([field()])} onDecide={vi.fn()} />);

    expect(screen.getByText("Security headers")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /accept as baseline/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /dismiss/i })).not.toBeInTheDocument();
  });

  it("keeps a matching family to one line, so the card is about what changed", () => {
    render(<SiteBaselineCard baseline={baseline([field()])} onDecide={vi.fn()} />);

    expect(screen.queryByText("x-frame-options: DENY")).not.toBeInTheDocument();
    expect(screen.queryByText("Recorded as good")).not.toBeInTheDocument();
  });

  it("shows what was recorded beside what is there now", () => {
    render(<SiteBaselineCard baseline={baseline([changedField])} onDecide={vi.fn()} />);

    expect(screen.getByText("x-frame-options: DENY")).toBeInTheDocument();
    expect(screen.getByText("x-frame-options: SAMEORIGIN")).toBeInTheDocument();
  });

  it("asks before accepting, because accepting rewrites what good means", () => {
    const onDecide = vi.fn();
    render(<SiteBaselineCard baseline={baseline([changedField])} onDecide={onDecide} />);

    fireEvent.click(screen.getByRole("button", { name: "Accept as baseline" }));

    expect(onDecide).not.toHaveBeenCalled();
    expect(screen.getByText(/Accepting makes this the baseline/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Accept as baseline" }));
    expect(onDecide).toHaveBeenCalledWith(changedField, true);
  });

  it("cancelling the confirmation leaves the baseline alone", () => {
    const onDecide = vi.fn();
    render(<SiteBaselineCard baseline={baseline([changedField])} onDecide={onDecide} />);

    fireEvent.click(screen.getByRole("button", { name: "Accept as baseline" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onDecide).not.toHaveBeenCalled();
    expect(screen.queryByText(/Accepting makes this the baseline/i)).not.toBeInTheDocument();
  });

  it("dismisses without confirmation, because it changes nothing about good", () => {
    const onDecide = vi.fn();
    render(<SiteBaselineCard baseline={baseline([changedField])} onDecide={onDecide} />);

    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));

    expect(onDecide).toHaveBeenCalledWith(changedField, false);
  });

  it("does not offer local dismissal for a connected baseline", () => {
    render(
      <SiteBaselineCard
        baseline={baseline([{ ...changedField, canDismiss: false }])}
        onDecide={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Accept as baseline" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Dismiss" })).not.toBeInTheDocument();
  });

  it("keeps a silenced change acceptable and does not offer to silence it twice", () => {
    render(
      <SiteBaselineCard
        baseline={baseline([{ ...changedField, status: "silenced" }])}
        onDecide={vi.fn()}
      />,
    );

    expect(screen.getByText("Change dismissed, baseline unchanged")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Accept as baseline" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Dismiss" })).not.toBeInTheDocument();
  });

  it("surfaces a refused decision as an alert rather than swallowing it", () => {
    render(
      <SiteBaselineCard
        baseline={baseline([changedField])}
        refusal="The site changed again while this was open."
        onDecide={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("The site changed again");
  });
});
