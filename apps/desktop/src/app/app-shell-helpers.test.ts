import { describe, expect, it, vi } from "vitest";
import { createProjectDeletedHandler } from "./app-shell-helpers";

describe("createProjectDeletedHandler", () => {
  it("refreshes projects, then lands on the dashboard so the switch is visible", async () => {
    const order: string[] = [];
    const refreshProjects = vi.fn(async () => {
      order.push("refresh");
    });
    const navigateTo = vi.fn((page: "dashboard") => {
      order.push(`navigate:${page}`);
    });

    await createProjectDeletedHandler({ refreshProjects, navigateTo })();

    expect(order).toEqual(["refresh", "navigate:dashboard"]);
  });
});
