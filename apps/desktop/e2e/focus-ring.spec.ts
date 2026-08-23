import { test, expect, type Page } from "@playwright/test";
import { installTauriStub } from "./fixtures/tauri-stub";
import { seededProjectResponses } from "./fixtures/seeded-project";
import { collectConsoleErrors } from "./fixtures/console-errors";
import { expectNoAccessibilityViolations } from "./fixtures/accessibility";

/**
 * Computed-style coverage for the primary-button keyboard focus ring
 * (fix-round-2 review of Task 9): base rules in cards.css, interactive.css,
 * and layout.css suppressed box-shadow across every state, including
 * :focus-visible, so tabbing to these buttons showed no indicator at all.
 * The accessibility fixture's axe pass cannot catch this: axe's
 * color-contrast and related checks run against the DOM's resting/hover
 * paint, not against a synthetic :focus-visible state, so a missing focus
 * ring needs its own computed-style assertion.
 */

async function focusViaKeyboard(page: Page, name: string | RegExp) {
  const target = page.getByRole("button", { name });
  await target.focus();
  // A direct .focus() call does not reliably put Chromium into keyboard
  // (:focus-visible) mode. Tabbing away and back does: the button's focus
  // then comes from a real keydown, which is exactly what :focus-visible
  // requires.
  await page.keyboard.press("Tab");
  await page.keyboard.press("Shift+Tab");
  return target;
}

function assertVisibleRing(boxShadow: string, label: string) {
  expect(boxShadow, `${label} has no box-shadow at all`).not.toBe("none");
  expect(boxShadow, `${label}'s box-shadow names no color`).toMatch(/rgb/);
  expect(boxShadow, `${label}'s box-shadow color is fully transparent`).not.toMatch(
    /rgba\([^)]*,\s*0\s*\)/,
  );
}

test.describe("keyboard focus ring", () => {
  test("renders a visible ring on the TopBar Run Scan button", async ({ page }) => {
    await installTauriStub(page, seededProjectResponses());
    const errors = collectConsoleErrors(page);

    await page.goto("/", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { level: 1, name: "Site Dashboard" })).toBeVisible({
      timeout: 15_000,
    });

    const runScan = await focusViaKeyboard(page, "Run Scan");
    const boxShadow = await runScan.evaluate((el) => getComputedStyle(el).boxShadow);
    assertVisibleRing(boxShadow, "Run Scan button");

    await expectNoAccessibilityViolations(page, "dashboard, Run Scan focused");
    expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toHaveLength(0);
  });

  test("renders a visible ring on the Reports page Generate Report button", async ({ page }) => {
    await installTauriStub(page, seededProjectResponses());
    const errors = collectConsoleErrors(page);

    await page.goto("/", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { level: 1, name: "Site Dashboard" })).toBeVisible({
      timeout: 15_000,
    });
    await page.getByRole("button", { name: "Reports" }).first().click();
    await expect(page.getByRole("heading", { level: 1, name: "Reports" })).toBeVisible();

    const generateReport = await focusViaKeyboard(page, /Generate Report/i);
    const boxShadow = await generateReport.evaluate((el) => getComputedStyle(el).boxShadow);
    assertVisibleRing(boxShadow, "Generate Report button");

    await expectNoAccessibilityViolations(page, "Reports page, Generate Report focused");
    expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toHaveLength(0);
  });
});
