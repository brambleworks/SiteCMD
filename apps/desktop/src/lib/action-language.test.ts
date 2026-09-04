import { describe, expect, it } from "vitest";

import {
  getCopyActionLabel,
  getLifecycleActionLabel,
  getOpenTargetLabel,
  getSummaryTargetLabel,
  getVerificationActionLabel,
  getWebCategoryOpenLabel,
  getWorkQueueActionLabel,
} from "./action-language";

describe("action language", () => {
  it("maps destination labels consistently", () => {
    expect(
      getOpenTargetLabel({
        page: "search-console",
        focus: null,
        scanKind: null,
        scanId: null,
        sessionId: null,
      }),
    ).toBe("Open Search & SEO");
    expect(
      getOpenTargetLabel({
        page: "issues",
        focus: "code-scan",
        scanKind: null,
        scanId: null,
        sessionId: null,
      }),
    ).toBe("Open Code Scan");
    expect(
      getOpenTargetLabel({
        page: "issues",
        focus: null,
        scanKind: null,
        scanId: null,
        sessionId: null,
      }),
    ).toBe("Open Issues");
    expect(
      getOpenTargetLabel({
        page: "issues",
        focus: null,
        scanKind: "code",
        scanId: 12,
        sessionId: null,
      }),
    ).toBe("Open Code Scan");
    expect(
      getOpenTargetLabel({
        page: "issues",
        focus: null,
        scanKind: "site",
        scanId: 12,
        sessionId: null,
      }),
    ).toBe("Open Results");
    expect(
      getOpenTargetLabel({
        page: "issues",
        focus: "code-scan-domain:database",
        scanKind: null,
        scanId: null,
        sessionId: null,
      }),
    ).toBe("Open Code Scan");
    expect(
      getOpenTargetLabel({
        page: "sites",
        focus: null,
        scanKind: null,
        scanId: null,
        sessionId: null,
      }),
    ).toBe("Open Overview");
    expect(
      getOpenTargetLabel({
        page: "issues",
        focus: null,
        scanKind: "site",
        scanId: null,
        sessionId: null,
        reason: "scan-after-deploy",
      }),
    ).toBe("Scan after Deploy");
    expect(
      getOpenTargetLabel({
        page: "deploys",
        focus: null,
        scanKind: null,
        scanId: null,
        sessionId: null,
        reason: "deploy-regression",
      }),
    ).toBe("Review Deploy Regression");
    expect(
      getOpenTargetLabel({
        page: "updates",
        focus: null,
        scanKind: null,
        scanId: null,
        sessionId: null,
        reason: "changed-dependencies",
      }),
    ).toBe("Refresh Updates");
    expect(
      getOpenTargetLabel({
        page: "updates",
        focus: null,
        scanKind: null,
        scanId: null,
        sessionId: null,
        itemId: "npm:react",
      }),
    ).toBe("Open Package Update");
    expect(
      getOpenTargetLabel({
        page: "search-console",
        focus: null,
        scanKind: null,
        scanId: null,
        sessionId: null,
        reason: "changed-search-file",
      }),
    ).toBe("Verify Search & SEO");
    expect(
      getOpenTargetLabel({
        page: "issues",
        focus: "security",
        scanKind: null,
        scanId: null,
        sessionId: null,
        reason: "changed-security-file",
      }),
    ).toBe("Verify Security");
  });

  it("uses summary-first labels for multi-package update contexts", () => {
    expect(getSummaryTargetLabel({ page: "updates", itemId: "npm:react" }, { itemCount: 3 })).toBe(
      "Open Updates",
    );
    expect(getSummaryTargetLabel({ page: "updates", itemId: "npm:react" }, { itemCount: 1 })).toBe(
      "Open Package Update",
    );
    expect(getSummaryTargetLabel({ page: "search-console" }, { itemCount: 3 })).toBe(
      "Open Search & SEO",
    );
  });

  it("maps verify labels consistently", () => {
    expect(getVerificationActionLabel()).toBe("Verify now");
    expect(getVerificationActionLabel({ repeated: true })).toBe("Verify again");
  });

  it("maps lifecycle button labels consistently", () => {
    expect(getLifecycleActionLabel("working")).toBe("Mark Working");
    expect(getLifecycleActionLabel("ignored")).toBe("Ignore");
    expect(getLifecycleActionLabel("blocked")).toBe("Block");
    expect(getLifecycleActionLabel("reopened")).toBe("Reopen");
  });

  it("maps web category destinations consistently", () => {
    expect(getWebCategoryOpenLabel("security")).toBe("Open Security Issues");
    expect(getWebCategoryOpenLabel("seo")).toBe("Open Search & SEO");
    expect(getWebCategoryOpenLabel("performance")).toBe("Open Performance Results");
    expect(getWebCategoryOpenLabel("accessibility")).toBe("Open Accessibility Results");
    expect(getWebCategoryOpenLabel("polish")).toBe("Open Polish Results");
    expect(getWebCategoryOpenLabel("compliance")).toBe("Open Privacy Results");
    expect(getWebCategoryOpenLabel("legal")).toBe("Open Privacy Results");
    expect(getWebCategoryOpenLabel("unknown")).toBe("Open Issues");
  });

  it("maps copy labels consistently", () => {
    expect(getCopyActionLabel("fix-bundle")).toBe("Copy Fix Bundle");
    expect(getCopyActionLabel("ai-task")).toBe("Copy AI Task");
    expect(getCopyActionLabel("ai-task", { subject: "Security" })).toBe("Copy Security AI Task");
    expect(getCopyActionLabel("commands")).toBe("Copy Commands");
    expect(getCopyActionLabel("fix-plan")).toBe("Copy Fix Plan");
    expect(getCopyActionLabel("fix-plan", { subject: "Code Scan" })).toBe(
      "Copy Code Scan Fix Plan",
    );
    expect(getCopyActionLabel("fix-steps")).toBe("Copy Fix Steps");
    expect(getCopyActionLabel("fix-prompt")).toBe("Copy Fix Prompt");
    expect(getCopyActionLabel("proof-checklist")).toBe("Copy Verification Checklist");
    expect(getCopyActionLabel("source-evidence")).toBe("Copy Source Evidence");
    expect(getCopyActionLabel("patch-prompt")).toBe("Copy Patch Prompt");
    expect(getCopyActionLabel("patch-prompt", { subject: "SEO" })).toBe("Copy SEO Patch Prompt");
    expect(getCopyActionLabel("command")).toBe("Copy Command");
    expect(getCopyActionLabel("prompt")).toBe("Copy Prompt");
    expect(getCopyActionLabel("fix-plan", { copied: true })).toBe("Copied Fix Plan");
    expect(getCopyActionLabel("ai-task", { copied: true })).toBe("Copied AI Task");
    expect(getCopyActionLabel("ai-task", { copied: true, subject: "Updates" })).toBe(
      "Copied Updates AI Task",
    );
  });

  it("maps work queue labels consistently", () => {
    expect(
      getWorkQueueActionLabel("resume", {
        kind: "code",
        status: "working",
        target: { page: "issues", scanKind: "code" },
      } as never),
    ).toBe("Resume");
    expect(
      getWorkQueueActionLabel("verify", {
        kind: "web",
        status: "new",
        target: { page: "search-console" },
      } as never),
    ).toBe("Verify now");
    expect(
      getWorkQueueActionLabel("fix", {
        kind: "update",
        status: "new",
        target: { page: "updates", itemId: "npm:react" },
      } as never),
    ).toBe("Open Package Update");
    expect(
      getWorkQueueActionLabel("fix", {
        kind: "update",
        status: "new",
        target: { page: "updates" },
      } as never),
    ).toBe("Open Updates");
    expect(
      getWorkQueueActionLabel("maintenance", {
        kind: "launch",
        status: "blocked",
        target: { page: "issues" },
      } as never),
    ).toBe("Resolve Block");

    expect(
      getWorkQueueActionLabel("maintenance", {
        kind: "web",
        status: "new",
        target: { page: "issues", reason: "no-first-scan" },
      } as never),
    ).toBe("Run First Web Scan");
    expect(
      getWorkQueueActionLabel("maintenance", {
        kind: "web",
        status: "new",
        target: { page: "issues", reason: "stale-web-scan" },
      } as never),
    ).toBe("Refresh Web Scan");
    expect(
      getWorkQueueActionLabel("maintenance", {
        kind: "code",
        status: "new",
        target: { page: "issues", reason: "stale-code-scan", scanKind: "code" },
      } as never),
    ).toBe("Refresh Code Scan");
    expect(
      getWorkQueueActionLabel("maintenance", {
        kind: "web",
        status: "new",
        target: { page: "issues", reason: "scan-after-deploy", scanKind: "site" },
      } as never),
    ).toBe("Scan after Deploy");
    expect(
      getWorkQueueActionLabel("maintenance", {
        kind: "update",
        status: "new",
        target: { page: "updates", reason: "changed-dependencies" },
      } as never),
    ).toBe("Refresh Updates");
    expect(
      getWorkQueueActionLabel("maintenance", {
        kind: "web",
        status: "new",
        target: { page: "search-console", reason: "changed-search-file" },
      } as never),
    ).toBe("Verify Search & SEO");
  });
});
