import { test, expect } from "@playwright/test";
import {
  DEFERRED,
  emitTauriEvent,
  installTauriStub,
  resolveDeferredInvoke,
  waitForTauriListener,
} from "./fixtures/tauri-stub";
import {
  runScanExecutionResult,
  seededDetail,
  seededProjectResponses,
} from "./fixtures/seeded-project";
import { collectConsoleErrors } from "./fixtures/console-errors";
import { expectNoAccessibilityViolations } from "./fixtures/accessibility";
import type { ScanResult } from "../src/generated/ipc-bindings";

// Drives a deferred web scan through progress events to its completion summary.

const completedScan: ScanResult = {
  ...seededDetail,
  overallScore: 84,
  durationMs: 1500,
  timestamp: "2026-04-16T09:30:00Z",
};

test.describe("web scan flow", () => {
  test("runs a scan through progress events to the summary overlay", async ({ page }) => {
    await installTauriStub(page, {
      ...seededProjectResponses(),
      run_scan_execution: DEFERRED,
    });
    const errors = collectConsoleErrors(page);

    await page.goto("/", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { level: 1, name: "Site Dashboard" })).toBeVisible({
      timeout: 15_000,
    });

    await page.getByRole("button", { name: "Run Scan" }).click();

    // The live overlay opens while run_scan_execution is pending.
    const overlay = page.getByRole("dialog", { name: "Scan in progress" });
    await expect(overlay).toBeVisible();
    await expectNoAccessibilityViolations(page, "scan overlay");

    // useScan attaches the progress listener after the overlay renders;
    // emitting before it exists would drop the event.
    await waitForTauriListener(page, "scan-progress");

    await emitTauriEvent(page, "scan-progress", {
      check_id: "security.headers",
      category: "security",
      status: "running",
      results_count: 0,
      checks_done: 3,
      checks_total: 10,
    });
    await expect(overlay.getByText("3 of 10 checks")).toBeVisible();

    await emitTauriEvent(page, "scan-progress", {
      check_id: "seo.sitemap",
      category: "seo",
      status: "complete",
      results_count: 1,
      checks_done: 10,
      checks_total: 10,
    });
    await expect(overlay.getByText("10 of 10 checks")).toBeVisible();

    // Backend finishes: the overlay closes and the summary takes over.
    await resolveDeferredInvoke(page, "run_scan_execution", runScanExecutionResult(completedScan));
    await expect(overlay).toBeHidden();

    const reviewIssues = page.getByRole("button", { name: "Review Issues" });
    await expect(reviewIssues).toBeVisible();
    // The completion toast runs a 200ms slide-in fade; scanning axe mid-transition
    // reads its still-transparent text as a false color-contrast violation.
    await expect(page.locator(".toast-item")).toHaveCSS("opacity", "1");
    await expectNoAccessibilityViolations(page, "scan summary");
    await reviewIssues.click();
    await expect(page.getByRole("heading", { level: 1, name: "Issues" })).toBeVisible();

    expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toHaveLength(0);
  });
});
