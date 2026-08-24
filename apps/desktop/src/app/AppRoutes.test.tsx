import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { InactiveProjectRoutes } from "./AppRoutes";

describe("InactiveProjectRoutes", () => {
  it("offers Add Project instead of a dead-end sentence", () => {
    const openAddProject = vi.fn();
    render(
      <InactiveProjectRoutes
        page="issues"
        onOpenOverviewProject={vi.fn()}
        openAddProject={openAddProject}
      />,
    );

    expect(screen.getByText("Select a project to get started")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add Project" }));
    expect(openAddProject).toHaveBeenCalledTimes(1);
  });
});
