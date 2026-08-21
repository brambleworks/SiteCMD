import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/logger", () => ({
  installGlobalErrorHandlers: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-deep-link", () => ({
  getCurrent: vi.fn(),
  onOpenUrl: vi.fn(),
}));

import {
  getLatestDeepLinkEnvelope,
  getProjectBootstrapState,
  getLatestDeepLinkTarget,
  handleDesktopNotificationAction,
  shouldIgnoreRepeatedDeepLink,
  shouldDeferAppTargetUntilProjectsReady,
} from "@/app/app-shell-helpers";
import {
  buildPostScanFollowUpBanner,
  getPreferredPostScanTarget,
  getWorkflowNotificationFollowUpAction,
} from "@/lib/scan-follow-up";

describe("getLatestDeepLinkTarget", () => {
  it("returns null for empty input", () => {
    expect(getLatestDeepLinkTarget([])).toBeNull();
    expect(getLatestDeepLinkTarget(null)).toBeNull();
  });

  it("preserves exact scan context from the newest valid deep link", () => {
    expect(
      getLatestDeepLinkTarget([
        "sitecmd://open?page=issues&projectId=1&url=https://example.com&scanId=11&scanKind=site",
        "sitecmd://open?page=issues&projectId=1&url=https://example.com&scanId=88&scanKind=code&itemId=db-owner-scope&focus=code-scan-domain:database",
      ]),
    ).toEqual({
      page: "issues",
      projectId: 1,
      url: "https://example.com",
      scanId: 88,
      sessionId: null,
      scanKind: "code",
      focus: "code-scan-domain:database",
      itemId: "db-owner-scope",
      promptId: null,
      lane: null,
      reason: null,
      filePath: null,
      restoreScan: false,
    });
  });

  it("prefers the last valid deep link when later entries are newer", () => {
    expect(
      getLatestDeepLinkTarget([
        "not-a-url",
        "sitecmd://open?page=dashboard",
        "sitecmd://open?page=issues&projectId=1&url=https://example.com&sessionId=44",
      ]),
    ).toEqual({
      page: "issues",
      projectId: 1,
      url: "https://example.com",
      scanId: null,
      sessionId: 44,
      scanKind: null,
      focus: null,
      itemId: null,
      promptId: null,
      lane: null,
      reason: null,
      filePath: null,
      restoreScan: false,
    });
  });
});

describe("getLatestDeepLinkEnvelope", () => {
  it("returns a canonical dedupe key for the newest valid deep link", () => {
    expect(
      getLatestDeepLinkEnvelope([
        "sitecmd://open?page=dashboard",
        "sitecmd://open?page=issues&projectId=1&url=https://example.com/&scanId=11",
      ]),
    ).toEqual({
      target: {
        page: "issues",
        projectId: 1,
        url: "https://example.com",
        scanId: 11,
        sessionId: null,
        scanKind: null,
        focus: null,
        itemId: null,
        promptId: null,
        lane: null,
        reason: null,
        filePath: null,
        restoreScan: false,
      },
      dedupeKey: JSON.stringify({
        page: "issues",
        projectId: 1,
        url: "https://example.com",
        scanId: 11,
        sessionId: null,
        scanKind: null,
        focus: null,
        itemId: null,
        promptId: null,
        lane: null,
        reason: null,
        filePath: null,
        restoreScan: false,
      }),
    });
  });
});

describe("shouldIgnoreRepeatedDeepLink", () => {
  it("ignores the same deep link when it repeats immediately", () => {
    expect(
      shouldIgnoreRepeatedDeepLink({
        nextKey: '{"page":"dashboard"}',
        lastKey: '{"page":"dashboard"}',
        elapsedMs: 250,
      }),
    ).toBe(true);
  });

  it("allows the same deep link again after the dedupe window", () => {
    expect(
      shouldIgnoreRepeatedDeepLink({
        nextKey: '{"page":"dashboard"}',
        lastKey: '{"page":"dashboard"}',
        elapsedMs: 5000,
      }),
    ).toBe(false);
  });

  it("allows a different deep link even within the dedupe window", () => {
    expect(
      shouldIgnoreRepeatedDeepLink({
        nextKey: '{"page":"scans"}',
        lastKey: '{"page":"dashboard"}',
        elapsedMs: 250,
      }),
    ).toBe(false);
  });
});

describe("getProjectBootstrapState", () => {
  it("shows a loading shell while the initial project bootstrap is still in flight", () => {
    expect(
      getProjectBootstrapState({
        projectCount: 0,
        projectsLoading: true,
        projectsLoadError: null,
        showAddProject: false,
      }),
    ).toBe("loading");
  });

  it("shows an error shell when startup fails before any projects load", () => {
    expect(
      getProjectBootstrapState({
        projectCount: 0,
        projectsLoading: false,
        projectsLoadError: "boom",
        showAddProject: false,
      }),
    ).toBe("error");
  });

  it("falls back to the welcome shell only after startup completed cleanly", () => {
    expect(
      getProjectBootstrapState({
        projectCount: 0,
        projectsLoading: false,
        projectsLoadError: null,
        showAddProject: false,
      }),
    ).toBe("welcome");
  });

  it("keeps the bootstrap shells out of the way once the add-project flow is open", () => {
    expect(
      getProjectBootstrapState({
        projectCount: 0,
        projectsLoading: true,
        projectsLoadError: "boom",
        showAddProject: true,
      }),
    ).toBeNull();
  });
});

describe("shouldDeferAppTargetUntilProjectsReady", () => {
  it("defers project-scoped targets until startup projects are ready", () => {
    expect(
      shouldDeferAppTargetUntilProjectsReady({
        projectCount: 0,
        projectsLoading: true,
        target: {
          page: "issues",
          projectId: 7,
          url: "https://example.com",
        },
      }),
    ).toBe(true);
  });

  it("allows global targets through immediately even during startup", () => {
    expect(
      shouldDeferAppTargetUntilProjectsReady({
        projectCount: 0,
        projectsLoading: true,
        target: {
          page: "settings",
        },
      }),
    ).toBe(false);
  });

  it("stops deferring once the project list is available", () => {
    expect(
      shouldDeferAppTargetUntilProjectsReady({
        projectCount: 1,
        projectsLoading: false,
        target: {
          page: "issues",
          projectId: 7,
          url: "https://example.com",
        },
      }),
    ).toBe(false);
  });
});

describe("handleDesktopNotificationAction", () => {
  it("forwards both file opens and exact app targets", async () => {
    const openFilePath = vi.fn(() => Promise.resolve());
    const openTarget = vi.fn();

    await handleDesktopNotificationAction(
      {
        sourceId: "scan:1",
        actionId: "open-results",
        filePath: "/tmp/session-report.json",
        target: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          sessionId: 44,
        },
      },
      { openFilePath, openTarget },
    );

    expect(openFilePath).toHaveBeenCalledWith("/tmp/session-report.json");
    expect(openTarget).toHaveBeenCalledWith({
      page: "issues",
      projectId: 1,
      url: "https://example.com",
      sessionId: 44,
    });
  });

  it("handles payloads that only contain a target", async () => {
    const openTarget = vi.fn();

    await handleDesktopNotificationAction(
      {
        sourceId: "scan:2",
        actionId: "open-results",
        target: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 77,
          scanKind: "site",
        },
      },
      { openTarget },
    );

    expect(openTarget).toHaveBeenCalledWith({
      page: "issues",
      projectId: 1,
      url: "https://example.com",
      scanId: 77,
      scanKind: "site",
    });
  });

  it("preserves exact code-scan focus targets from desktop notification actions", async () => {
    const openTarget = vi.fn();

    await handleDesktopNotificationAction(
      {
        sourceId: "scan:code-1",
        actionId: "open-results",
        target: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 91,
          scanKind: "code",
          focus: "code-scan-domain:database",
          itemId: "db-owner-scope",
          filePath: "/tmp/example/db.ts",
        },
      },
      { openTarget },
    );

    expect(openTarget).toHaveBeenCalledWith({
      page: "issues",
      projectId: 1,
      url: "https://example.com",
      scanId: 91,
      scanKind: "code",
      focus: "code-scan-domain:database",
      itemId: "db-owner-scope",
      filePath: "/tmp/example/db.ts",
    });
  });

  it("preserves exact package update targets from desktop notification actions", async () => {
    const openTarget = vi.fn();

    await handleDesktopNotificationAction(
      {
        sourceId: "updates:react",
        actionId: "open-update",
        target: {
          page: "updates",
          projectId: 15,
          url: "https://deps.test",
          itemId: "npm:react",
        },
      },
      { openTarget },
    );

    expect(openTarget).toHaveBeenCalledWith({
      page: "updates",
      projectId: 15,
      url: "https://deps.test",
      itemId: "npm:react",
    });
  });
});

describe("getWorkflowNotificationFollowUpAction", () => {
  it("returns a follow-up action when the workflow target differs from the results target", () => {
    expect(
      getWorkflowNotificationFollowUpAction(
        {
          label: "1 regressed",
          sentence: "Resume 1 regressed item next.",
          target: {
            page: "issues",
            projectId: 1,
            url: "https://example.com",
            reason: "changed-security-file",
          },
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 88,
          scanKind: "site",
        },
      ),
    ).toEqual({
      id: "resume-workflow",
      label: "Verify Security",
      target: {
        page: "issues",
        projectId: 1,
        url: "https://example.com",
        reason: "changed-security-file",
      },
    });
  });

  it("uses the exact package label for updates follow-up actions", () => {
    expect(
      getWorkflowNotificationFollowUpAction(
        {
          label: "1 working",
          sentence: "1 update is ready to verify.",
          target: {
            page: "updates",
            projectId: 15,
            url: "https://deps.test",
            itemId: "npm:react",
          },
        },
        {
          page: "issues",
          projectId: 15,
          url: "https://deps.test",
          scanId: 88,
          scanKind: "site",
        },
      ),
    ).toEqual({
      id: "resume-workflow",
      label: "Open Package Update",
      target: {
        page: "updates",
        projectId: 15,
        url: "https://deps.test",
        itemId: "npm:react",
      },
    });
  });

  it("skips the follow-up action when the workflow target is the same as the primary target", () => {
    expect(
      getWorkflowNotificationFollowUpAction(
        {
          label: "1 working",
          sentence: "1 in-progress item is ready to resume.",
          target: {
            page: "issues",
            projectId: 1,
            url: "https://example.com",
            scanId: 88,
            scanKind: "site",
          },
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 88,
          scanKind: "site",
        },
      ),
    ).toBeNull();
  });
});

describe("getPreferredPostScanTarget", () => {
  it("prefers urgent workflow targets over generic scan results", () => {
    expect(
      getPreferredPostScanTarget(
        {
          key: "launch-blockers",
          label: "1 launch blocker",
          sentence: "1 launch blocker is still open.",
          target: {
            page: "updates",
            projectId: 1,
            url: "https://example.com",
          },
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 55,
          scanKind: "site",
        },
      ),
    ).toEqual({
      page: "updates",
      projectId: 1,
      url: "https://example.com",
      scanId: null,
      sessionId: null,
      scanKind: null,
      focus: null,
      itemId: null,
      promptId: null,
      lane: null,
      reason: null,
      filePath: null,
    });
  });

  it("keeps blocked or ignored workflow cues from hijacking the post-scan landing", () => {
    expect(
      getPreferredPostScanTarget(
        {
          key: "blocked",
          label: "1 blocked",
          sentence: "1 blocked item needs a decision.",
          target: {
            page: "updates",
            projectId: 1,
            url: "https://example.com",
          },
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 55,
          scanKind: "site",
        },
      ),
    ).toEqual({
      page: "issues",
      projectId: 1,
      url: "https://example.com",
      scanId: 55,
      sessionId: null,
      scanKind: "site",
      focus: null,
      itemId: null,
      promptId: null,
      lane: null,
      reason: null,
      filePath: null,
    });
  });

  it("merges scan-focused workflow targets into the exact result that just finished", () => {
    expect(
      getPreferredPostScanTarget(
        {
          key: "regressed",
          label: "1 regressed",
          sentence: "Resume 1 regressed item next.",
          target: {
            page: "issues",
            projectId: 1,
            url: "https://example.com",
            focus: "code-scan-domain:database",
            itemId: "db-owner-scope",
          },
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 88,
          scanKind: "code",
        },
      ),
    ).toEqual({
      page: "issues",
      projectId: 1,
      url: "https://example.com",
      scanId: 88,
      sessionId: null,
      scanKind: "code",
      focus: "code-scan-domain:database",
      itemId: "db-owner-scope",
      promptId: null,
      lane: null,
      reason: null,
      filePath: null,
    });
  });
});

describe("buildPostScanFollowUpBanner", () => {
  it("builds a visible follow-up banner when the workflow target is stronger than raw results", () => {
    expect(
      buildPostScanFollowUpBanner(
        {
          key: "regressed",
          label: "1 regressed",
          sentence: "Resume 1 regressed item next.",
          target: {
            page: "issues",
            projectId: 1,
            url: "https://example.com",
            reason: "changed-security-file",
          },
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          reason: "changed-security-file",
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 55,
          scanKind: "site",
        },
      ),
    ).toMatchObject({
      title: "A regression needs attention",
      description: "Resume 1 regressed item next.",
      actionLabel: "Verify Security",
      target: {
        page: "issues",
        projectId: 1,
        url: "https://example.com",
        reason: "changed-security-file",
      },
    });
  });

  it("skips the banner when the chosen target is still the raw primary scan result", () => {
    expect(
      buildPostScanFollowUpBanner(
        {
          key: "working",
          label: "1 working",
          sentence: "1 in-progress item is ready to resume.",
          target: {
            page: "issues",
            projectId: 1,
            url: "https://example.com",
            scanId: 55,
            scanKind: "site",
          },
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 55,
          scanKind: "site",
        },
        {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 55,
          scanKind: "site",
        },
      ),
    ).toBeNull();
  });
});
