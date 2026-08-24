import { test, expect, type Locator, type Page } from "@playwright/test";
import { installTauriStub } from "./fixtures/tauri-stub";
import { seededProjectResponses } from "./fixtures/seeded-project";
import { collectConsoleErrors } from "./fixtures/console-errors";
import { expectNoAccessibilityViolations } from "./fixtures/accessibility";

/** Browser smoke coverage for boot and navigation with stubbed Tauri IPC. */

test.describe("first-run boot", () => {
  test("renders the welcome state without console errors", async ({ page }) => {
    await installTauriStub(page);
    const errors = collectConsoleErrors(page);

    await page.goto("/", { waitUntil: "domcontentloaded" });
    const heading = page.locator("h1, h2").filter({ visible: true }).first();
    await expect(heading).toBeVisible({ timeout: 15_000 });
    await expectNoAccessibilityViolations(page, "welcome");

    expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toHaveLength(0);
  });
});

test.describe("seeded navigation", () => {
  test("every sidebar page renders its own heading", async ({ page }) => {
    await installTauriStub(page, seededProjectResponses());
    const errors = collectConsoleErrors(page);

    await page.goto("/", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { level: 1, name: "Site Dashboard" })).toBeVisible({
      timeout: 15_000,
    });

    // `ready` targets page content rather than the shell heading, forcing lazy chunks to mount.
    const pages: Array<{ nav: string; heading: string; ready: (page: Page) => Locator }> = [
      {
        nav: "Issues",
        heading: "Issues",
        ready: (p) => p.getByRole("button", { name: "Pages" }),
      },
      {
        nav: "Alerts",
        heading: "Alerts",
        ready: (p) => p.getByRole("button", { name: "Mark all read" }),
      },
      {
        nav: "Updates",
        heading: "Updates",
        ready: (p) => p.getByRole("button", { name: "Add Folder" }),
      },
      {
        nav: "Integrations",
        heading: "Integrations",
        ready: (p) => p.getByText("Analytics & Monitoring"),
      },
      {
        nav: "Activity",
        heading: "Activity",
        ready: (p) => p.getByRole("button", { name: "Export CSV" }),
      },
      {
        nav: "Reports",
        heading: "Reports",
        ready: (p) => p.getByRole("button", { name: /Generate Report/i }),
      },
      {
        nav: "Dashboard",
        heading: "Site Dashboard",
        ready: (p) => p.getByText("Recent Activity"),
      },
    ];
    for (const { nav, heading, ready } of pages) {
      const navItem = page.getByRole("button", { name: nav }).first();
      await expect(navItem, `nav item "${nav}" must exist for a seeded project`).toBeVisible();
      await navItem.click();
      await expect(page.getByRole("heading", { level: 1, name: heading })).toBeVisible();
      await expect(ready(page), `page "${nav}" must mount its content`).toBeVisible({
        timeout: 15_000,
      });
      await expectNoAccessibilityViolations(page, `${nav} page`);
    }

    // Settings lives on the sidebar utility bar as an icon button.
    await page.getByRole("button", { name: "Settings" }).first().click();
    await expect(page.getByRole("heading", { level: 1, name: "Project Settings" })).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Select Folder" }),
      "settings page must mount its content",
    ).toBeVisible({ timeout: 15_000 });
    await expectNoAccessibilityViolations(page, "settings page");

    expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toHaveLength(0);
  });
});
