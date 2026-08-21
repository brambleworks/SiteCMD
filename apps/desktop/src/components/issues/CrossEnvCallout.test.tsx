import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { CrossEnvCallout } from "./CrossEnvCallout";
import type { CrossEnvSignal } from "@/lib/types";

describe("CrossEnvCallout", () => {
  it("renders nothing when signal is null", () => {
    const { container } = render(<CrossEnvCallout signal={null} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders signal message with daysBeforeProd", () => {
    const signal: CrossEnvSignal = {
      stagingObservedAt: "2024-01-15T10:00:00Z",
      daysBeforeProd: 3,
    };
    render(<CrossEnvCallout signal={signal} />);
    expect(screen.getByText(/3 days ago/)).toBeInTheDocument();
    expect(screen.getByText(/non-production environment/)).toBeInTheDocument();
  });

  it("uses singular 'day' when daysBeforeProd is 1", () => {
    const signal: CrossEnvSignal = {
      stagingObservedAt: "2024-01-15T10:00:00Z",
      daysBeforeProd: 1,
    };
    render(<CrossEnvCallout signal={signal} />);
    expect(screen.getByText(/1 day ago/)).toBeInTheDocument();
  });

  it("uses plural 'days' when daysBeforeProd > 1", () => {
    const signal: CrossEnvSignal = {
      stagingObservedAt: "2024-01-15T10:00:00Z",
      daysBeforeProd: 5,
    };
    render(<CrossEnvCallout signal={signal} />);
    expect(screen.getByText(/5 days ago/)).toBeInTheDocument();
  });

  it("renders label 'Predicted from staging'", () => {
    const signal: CrossEnvSignal = {
      stagingObservedAt: "2024-01-15T10:00:00Z",
      daysBeforeProd: 2,
    };
    render(<CrossEnvCallout signal={signal} />);
    expect(screen.getByText("Predicted from staging")).toBeInTheDocument();
  });
});
