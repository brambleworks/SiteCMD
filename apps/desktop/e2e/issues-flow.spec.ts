import { test, expect } from "@playwright/test";
import { installTauriStub } from "./fixtures/tauri-stub";
import {
  SEEDED_ISSUE_TITLE,
  SEEDED_SCORE,
  seededProjectResponses,
} from "./fixtures/seeded-project";
import { collectConsoleErrors } from "./fixtures/console-errors";
import { expectNoAccessibilityViolations } from "./fixtures/accessibility";

// Typed seeded-project flow from dashboard score to its failing issue.

test.describe("seeded project flow", () => {
  let errors: string[];

  test.beforeEach(async ({ page }) => {
    await installTauriStub(page, seededProjectResponses());
    errors = collectConsoleErrors(page);
    await page.goto("/", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { level: 1, name: "Site Dashboard" })).toBeVisible({
      timeout: 15_000,
    });
  });

  test("dashboard renders the seeded score on the score tile", async ({ page }) => {
    // Scope the score assertion to its tile.
    const scoreTile = page.getByRole("button").filter({ hasText: "SiteCMD Score" }).first();
    await expect(scoreTile).toBeVisible();
    await expect(scoreTile).toContainText(String(SEEDED_SCORE));
    await expectNoAccessibilityViolations(page, "dashboard");

    expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toHaveLength(0);
  });

  test("issues page lists the seeded failing check", async ({ page }) => {
    await page.getByRole("button", { name: "Issues" }).first().click();
    await expect(page.getByRole("heading", { level: 1, name: "Issues" })).toBeVisible();

    // The seeded high-severity web issue renders as an open row with its
    // title and severity label.
    const issueRow = page.getByRole("button").filter({ hasText: SEEDED_ISSUE_TITLE }).first();
    await expect(issueRow).toBeVisible();
    await expect(issueRow).toContainText("High");
    await expectNoAccessibilityViolations(page, "issues list");

    expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toHaveLength(0);
  });
});
