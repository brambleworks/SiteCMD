import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@/lib/tauri-invoke", () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }));

import {
  ACTIVE_ATTEMPT_STATUSES,
  createFixAttempt,
  getFixAttemptForIssue,
  isAttemptActive,
  type CreateFixAttemptInput,
  type FixAttemptStatus,
} from "./fix-attempts";

const sampleInput: CreateFixAttemptInput = {
  projectId: 3,
  envUrl: "https://example.com",
  checkId: "security.csp",
  agentTool: "claude-code",
  title: "Missing Content-Security-Policy",
  severity: "high",
  description: "The site does not send a Content-Security-Policy header.",
  url: "https://example.com",
};

beforeEach(() => {
  invokeMock.mockReset();
});

describe("createFixAttempt", () => {
  it("forwards a structured args payload", async () => {
    invokeMock.mockResolvedValue({
      id: 42,
      status: "briefed",
      agentTool: "claude-code",
      agentSummary: null,
      failureDetail: null,
      kickoffPrompt: "prompt",
      createdAt: 1,
      updatedAt: 1,
    });

    const attempt = await createFixAttempt(sampleInput);

    expect(invokeMock).toHaveBeenCalledWith("create_fix_attempt", {
      args: expect.objectContaining({
        projectId: 3,
        checkId: "security.csp",
        agentTool: "claude-code",
      }),
    });
    expect(attempt.id).toBe(42);
  });
});

describe("getFixAttemptForIssue", () => {
  it("passes the issue identity", async () => {
    invokeMock.mockResolvedValue(null);

    const attempt = await getFixAttemptForIssue(
      3,
      "https://example.com",
      "security.csp",
      "Missing Content-Security-Policy",
    );

    expect(invokeMock).toHaveBeenCalledWith("get_fix_attempt_for_issue", {
      projectId: 3,
      envUrl: "https://example.com",
      checkId: "security.csp",
      title: "Missing Content-Security-Policy",
    });
    expect(attempt).toBeNull();
  });
});

describe("isAttemptActive", () => {
  it("treats briefed and the verify in-flight statuses as active", () => {
    const active: FixAttemptStatus[] = ["briefed", "verify_requested", "verifying"];
    for (const status of active) {
      expect(isAttemptActive(status)).toBe(true);
    }
    expect(ACTIVE_ATTEMPT_STATUSES).toEqual(active);
  });

  it("treats terminal statuses as inactive", () => {
    const terminal: FixAttemptStatus[] = ["verified", "verify_failed", "canceled", "expired"];
    for (const status of terminal) {
      expect(isAttemptActive(status)).toBe(false);
    }
  });
});
