import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  buildObservabilitySnapshotText,
  clearObservabilitySnapshot,
  recordErrorReport,
  recordWorkflowHealthEvent,
} from "./observability";

describe("observability", () => {
  beforeEach(() => {
    clearObservabilitySnapshot();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-04-14T12:00:00.000Z"));
  });

  it("builds a workflow health snapshot with degraded signals", () => {
    recordWorkflowHealthEvent("add_site", "started", { mode: "folder" });
    recordWorkflowHealthEvent("add_site", "failed", { errorType: "limit" });
    recordWorkflowHealthEvent("run_scan", "succeeded", { kind: "health", durationMs: 1420 });
    recordWorkflowHealthEvent("open_issues", "failed", { hasError: true });

    const snapshot = buildObservabilitySnapshotText();

    expect(snapshot).toContain("SiteCMD Observability Snapshot");
    expect(snapshot).toContain("onboarding: needs attention");
    expect(snapshot).toContain("issues: needs attention");
    expect(snapshot).toContain("add site: 0 succeeded, 1 failed, 1 started");
    expect(snapshot).toContain("run scan: 1 succeeded, 0 failed, 0 started");
  });

  it("sanitizes urls, paths, emails, and secrets from error reports", () => {
    recordErrorReport(
      "startup.bootstrap",
      "Failed at https://example.com for /Users/dev/private/file.ts with admin@example.com token sk_secret_12345",
      { fatal: true },
    );

    const snapshot = buildObservabilitySnapshotText();

    expect(snapshot).toContain("[url]");
    expect(snapshot).toContain("[path]");
    expect(snapshot).toContain("[email]");
    expect(snapshot).toContain("[secret]");
    expect(snapshot).not.toContain("https://example.com");
    expect(snapshot).not.toContain("/Users/dev/private/file.ts");
    expect(snapshot).not.toContain("admin@example.com");
  });

  it("sanitizes persisted observability records before building snapshots", () => {
    window.localStorage.setItem(
      "sitecmd_observability_v1",
      JSON.stringify({
        workflow: [
          {
            kind: "workflow",
            name: "run_scan",
            status: "failed",
            timestamp: "2026-04-14T11:59:00.000Z",
            meta: {
              target: "https://example.com/reset?token=abc",
              path: "/Users/dev/private/site",
            },
          },
        ],
        errors: [
          {
            kind: "error",
            source: "startup.bootstrap",
            fatal: true,
            timestamp: "2026-04-14T12:00:00.000Z",
            message: "Failed for admin@example.com with sk_secret_12345",
            meta: {
              log: "see /private/tmp/sitecmd.log",
            },
          },
        ],
      }),
    );

    const snapshot = buildObservabilitySnapshotText();

    expect(snapshot).toContain("[url]");
    expect(snapshot).toContain("[path]");
    expect(snapshot).toContain("[email]");
    expect(snapshot).toContain("[secret]");
    expect(snapshot).not.toContain("https://example.com/reset");
    expect(snapshot).not.toContain("/Users/dev/private/site");
    expect(snapshot).not.toContain("admin@example.com");
    expect(snapshot).not.toContain("sk_secret_12345");
  });

  it("drops persisted error records with unknown sources", () => {
    window.localStorage.setItem(
      "sitecmd_observability_v1",
      JSON.stringify({
        errors: [
          {
            kind: "error",
            source: "custom.untrusted",
            fatal: true,
            timestamp: "2026-04-14T12:00:00.000Z",
            message: "should not render",
          },
        ],
      }),
    );

    const snapshot = buildObservabilitySnapshotText();

    expect(snapshot).toContain("No observability events captured yet.");
    expect(snapshot).not.toContain("custom.untrusted");
    expect(snapshot).not.toContain("should not render");
  });

  it("shows an empty state when nothing has been captured yet", () => {
    const snapshot = buildObservabilitySnapshotText();

    expect(snapshot).toContain("No observability events captured yet.");
  });
});
