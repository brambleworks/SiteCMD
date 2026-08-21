import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ProjectWorkItem } from "@/lib/project-summary-types";
import { PausedIssuesList } from "./IssuesIssueTabAuxiliary";

const BLOCKED_ITEM: ProjectWorkItem = {
  stableKey: "security.csp",
  projectId: 7,
  environmentUrl: "https://example.com",
  kind: "web",
  status: "blocked",
  severity: "high",
  title: "Content Security Policy is missing",
  summary: "The response does not include a CSP header.",
  category: "security",
  domain: null,
  packageName: null,
  target: {
    page: "issues",
    projectId: 7,
    url: "https://example.com",
    itemId: "security.csp",
  },
  firstSeenAt: "",
  lastSeenAt: "",
  lastVerifiedAt: null,
  lastStatusChangedAt: "",
};

describe("PausedIssuesList", () => {
  it("shows blocked issues with a direct Restore action", () => {
    const onRestore = vi.fn();
    render(
      <PausedIssuesList
        statusFilter="blocked"
        pausedWorkItems={[BLOCKED_ITEM]}
        restoringCheckId={null}
        onRestore={onRestore}
      />,
    );

    expect(screen.getByText(BLOCKED_ITEM.title)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: `Restore ${BLOCKED_ITEM.title}` }));
    expect(onRestore).toHaveBeenCalledWith("security.csp");
  });

  it("does not render paused rows in the active view", () => {
    const { container } = render(
      <PausedIssuesList
        statusFilter="active"
        pausedWorkItems={[BLOCKED_ITEM]}
        restoringCheckId={null}
        onRestore={vi.fn()}
      />,
    );

    expect(container).toBeEmptyDOMElement();
  });
});
