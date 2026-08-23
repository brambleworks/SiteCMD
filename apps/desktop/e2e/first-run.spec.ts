import { test, expect } from "@playwright/test";
import {
  DEFERRED,
  installTauriStub,
  resolveDeferredInvoke,
  setInvokeResponse,
  waitForDeferredInvoke,
} from "./fixtures/tauri-stub";
import {
  runScanExecutionResult,
  seededDetail,
  seededProject,
  seededProjectResponses,
} from "./fixtures/seeded-project";
import { collectConsoleErrors } from "./fixtures/console-errors";
import { expectNoAccessibilityViolations } from "./fixtures/accessibility";

// The whole first run: Add Project, the automatic baseline scan, the summary,
// landing on Issues with the tour, and only then the telemetry prompt.

test.describe("first run", () => {
  test("goes from Add Project to the Issues tour before asking for telemetry", async ({ page }) => {
    await installTauriStub(
      page,
      {
        ...seededProjectResponses(),
        get_projects: [],
        add_project_by_url: DEFERRED,
        run_scan_execution: DEFERRED,
      },
      { telemetryPrompt: "unseen" },
    );
    const errors = collectConsoleErrors(page);

    await page.goto("/", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { name: /Welcome to/ })).toBeVisible({
      timeout: 15_000,
    });
    await expectNoAccessibilityViolations(page, "welcome");

    await page.getByRole("button", { name: "Add Project" }).click();
    await page.locator("#project-name").fill(seededProject.name);
    await page.locator("#primary-url").fill("https://example.com");
    await expectNoAccessibilityViolations(page, "add project form");
    await page.getByRole("button", { name: "Create Project" }).click();

    // The backend now knows the project; publish it before the create call resolves.
    await waitForDeferredInvoke(page, "add_project_by_url");
    await setInvokeResponse(page, "get_projects", [seededProject]);
    await resolveDeferredInvoke(page, "add_project_by_url", seededProject.id);

    const overlay = page.getByRole("dialog", { name: "Scan in progress" });
    await expect(overlay).toBeVisible({ timeout: 15_000 });
    await resolveDeferredInvoke(
      page,
      "run_scan_execution",
      runScanExecutionResult({ ...seededDetail, timestamp: "2026-08-22T09:00:00Z" }),
    );
    await expect(overlay).toBeHidden();

    const reviewIssues = page.getByRole("button", { name: "Review Issues" });
    await expect(reviewIssues).toBeVisible();
    const consentHeading = page.getByRole("heading", { name: "Help improve SiteCMD" });
    await expect(consentHeading).toBeHidden();
    // The completion toast runs a 200ms slide-in fade; scanning axe mid-transition
    // reads its still-transparent text as a false color-contrast violation.
    await expect(page.locator(".toast-item")).toHaveCSS("opacity", "1");
    await expectNoAccessibilityViolations(page, "scan summary");

    await reviewIssues.click();
    await expect(page.getByRole("heading", { level: 1, name: "Issues" })).toBeVisible();
    const tour = page.getByRole("complementary", { name: "First run walkthrough" });
    await expect(tour).toBeVisible();
    await expect(tour).toContainText("Step 1 of 6");
    await expect(consentHeading).toBeHidden();
    await expectNoAccessibilityViolations(page, "issues with walkthrough");

    await page.getByRole("button", { name: "Close walkthrough" }).click();
    await expect(consentHeading).toBeVisible();
    await expectNoAccessibilityViolations(page, "telemetry consent");

    expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toHaveLength(0);
  });
});
