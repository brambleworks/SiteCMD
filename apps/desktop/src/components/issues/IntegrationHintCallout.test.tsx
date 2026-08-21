import { fireEvent, render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { IntegrationHintCallout } from "./IntegrationHintCallout";
import type { IntegrationSuggestion } from "@/lib/types";

const { emitMock } = vi.hoisted(() => ({ emitMock: vi.fn() }));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: unknown[]) => {
    emitMock(...args);
    return Promise.resolve();
  },
}));

describe("IntegrationHintCallout", () => {
  it("renders nothing when no suggestions", () => {
    const { container } = render(<IntegrationHintCallout projectId={1} suggestions={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders up to 2 suggestions with Connect CTAs", () => {
    const suggestions: IntegrationSuggestion[] = [
      {
        checkId: "performance.lcp",
        integration: "googlesearchconsole",
        valueProp: "See CrUX data",
      },
      { checkId: "performance.lcp", integration: "plausible", valueProp: "See affected pages" },
    ];
    render(<IntegrationHintCallout projectId={1} suggestions={suggestions} />);
    expect(screen.getAllByText(/Get more context/)).toHaveLength(2);
    expect(screen.getByRole("button", { name: /Connect Search Console/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Connect Plausible/ })).toBeInTheDocument();
  });

  it("calls onOpenIntegrations with the integration key on Connect click", () => {
    const onOpenIntegrations = vi.fn();
    const suggestions: IntegrationSuggestion[] = [
      {
        checkId: "performance.lcp",
        integration: "plausible",
        valueProp: "Track real users",
      },
    ];
    render(
      <IntegrationHintCallout
        projectId={1}
        suggestions={suggestions}
        onOpenIntegrations={onOpenIntegrations}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Connect Plausible/ }));
    expect(onOpenIntegrations).toHaveBeenCalledWith("plausible");
  });

  it("announces a dismissal so the issue groups refetch", async () => {
    const suggestions: IntegrationSuggestion[] = [
      { checkId: "performance.lcp", integration: "plausible", valueProp: "Track real users" },
    ];
    render(<IntegrationHintCallout projectId={4} suggestions={suggestions} />);

    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));

    await vi.waitFor(() =>
      expect(emitMock).toHaveBeenCalledWith("integration-hint-dismissed", {
        projectId: 4,
        checkId: "performance.lcp",
        integration: "plausible",
      }),
    );
  });
});
