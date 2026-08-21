import { describe, expect, it } from "vitest";

import {
  buildFileWatchNotificationActions,
  buildOpenTargetNotificationAction,
  buildScanResultNotificationActions,
} from "./notification-actions";

describe("notification action builders", () => {
  it("derives open-target labels from the shared action language", () => {
    expect(
      buildOpenTargetNotificationAction("open-security", {
        page: "issues",
        projectId: 1,
        url: "https://example.com",
        focus: "code-scan",
      }),
    ).toEqual({
      id: "open-security",
      label: "Open Code Scan",
      target: {
        page: "issues",
        projectId: 1,
        url: "https://example.com",
        focus: "code-scan",
      },
    });
  });

  it("uses the exact package label for targeted Updates actions", () => {
    expect(
      buildOpenTargetNotificationAction("open-update", {
        page: "updates",
        projectId: 7,
        url: "https://example.com",
        itemId: "npm:react",
      }),
    ).toEqual({
      id: "open-update",
      label: "Open Package Update",
      target: {
        page: "updates",
        projectId: 7,
        url: "https://example.com",
        itemId: "npm:react",
      },
    });
  });

  it("builds scan result actions with a shared primary open-results action", () => {
    expect(
      buildScanResultNotificationActions({
        primaryTarget: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 88,
          scanKind: "code",
        },
        secondaryAction: {
          id: "open-security",
          label: "Open Security",
          target: {
            page: "issues",
            projectId: 1,
            url: "https://example.com",
            focus: "code-scan",
          },
        },
      }),
    ).toEqual([
      {
        id: "open-results",
        label: "Open Code Scan",
        target: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 88,
          scanKind: "code",
        },
        filePath: null,
      },
      {
        id: "open-security",
        label: "Open Security",
        target: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          focus: "code-scan",
        },
        filePath: null,
      },
    ]);
  });

  it("builds file-watch actions with both file-open and verify shortcuts", () => {
    expect(
      buildFileWatchNotificationActions({
        filePath: "/tmp/app/api/route.ts",
        verifyTarget: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          focus: "sec.headers",
          promptId: "prompt-1",
          reason: "changed-security-file",
        },
      }),
    ).toEqual([
      {
        id: "open-file",
        label: "Open changed file",
        target: null,
        filePath: "/tmp/app/api/route.ts",
      },
      {
        id: "verify-now",
        label: "Verify Security",
        target: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          focus: "sec.headers",
          promptId: "prompt-1",
          reason: "changed-security-file",
        },
        filePath: null,
      },
    ]);
  });
});
