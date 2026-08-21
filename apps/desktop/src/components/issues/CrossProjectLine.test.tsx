import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { CrossProjectLine } from "./CrossProjectLine";
import type { CrossProjectPattern } from "@/lib/types";

describe("CrossProjectLine", () => {
  let realDateNow: typeof Date.now;

  beforeEach(() => {
    realDateNow = Date.now;
    // Mock Date.now to a fixed point for consistent relative time testing
    Date.now = vi.fn(() => new Date("2024-01-20T12:00:00Z").getTime());
  });

  afterEach(() => {
    Date.now = realDateNow;
  });

  it("renders nothing when pattern is null", () => {
    const { container } = render(<CrossProjectLine pattern={null} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders nothing when projectCount is 0", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 0,
      lastSeenAt: "2024-01-19T12:00:00Z",
    };
    const { container } = render(<CrossProjectLine pattern={pattern} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders singular 'project' when projectCount is 1", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 1,
      lastSeenAt: "2024-01-19T12:00:00Z",
    };
    render(<CrossProjectLine pattern={pattern} />);
    expect(screen.getByText(/1 other project/)).toBeInTheDocument();
  });

  it("renders plural 'projects' when projectCount > 1", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 5,
      lastSeenAt: "2024-01-15T12:00:00Z",
    };
    render(<CrossProjectLine pattern={pattern} />);
    expect(screen.getByText(/5 other projects/)).toBeInTheDocument();
  });

  it("uses compact relative time for same-day (hours)", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 2,
      lastSeenAt: "2024-01-20T08:00:00Z",
    };
    render(<CrossProjectLine pattern={pattern} />);
    expect(screen.getByText(/last seen 4h ago/)).toBeInTheDocument();
  });

  it("uses compact relative time '1d ago'", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 2,
      lastSeenAt: "2024-01-19T12:00:00Z",
    };
    render(<CrossProjectLine pattern={pattern} />);
    expect(screen.getByText(/last seen 1d ago/)).toBeInTheDocument();
  });

  it("uses compact relative time 'Nd ago' for multiple days", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 2,
      lastSeenAt: "2024-01-10T12:00:00Z",
    };
    render(<CrossProjectLine pattern={pattern} />);
    expect(screen.getByText(/last seen 10d ago/)).toBeInTheDocument();
  });

  it("uses compact relative time '1mo ago'", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 2,
      lastSeenAt: "2023-12-21T12:00:00Z",
    };
    render(<CrossProjectLine pattern={pattern} />);
    expect(screen.getByText(/last seen 1mo ago/)).toBeInTheDocument();
  });

  it("uses compact relative time 'Nmo ago' for multiple months", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 2,
      lastSeenAt: "2023-10-20T12:00:00Z",
    };
    render(<CrossProjectLine pattern={pattern} />);
    expect(screen.getByText(/last seen 3mo ago/)).toBeInTheDocument();
  });

  it("shows 'unknown' for invalid dates", () => {
    const pattern: CrossProjectPattern = {
      projectCount: 2,
      lastSeenAt: "not-a-date",
    };
    render(<CrossProjectLine pattern={pattern} />);
    expect(screen.getByText(/last seen unknown/)).toBeInTheDocument();
  });
});
