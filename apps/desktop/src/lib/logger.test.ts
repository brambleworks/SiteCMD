import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn(async () => null));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: invokeMock,
}));

vi.mock("@/lib/observability", () => ({
  recordErrorReport: vi.fn(),
}));

import { logger, sanitizeFrontendLogText } from "./logger";

describe("frontend logger", () => {
  beforeEach(() => {
    invokeMock.mockClear();
  });

  it("redacts urls, paths, emails, and secrets before sending logs to Rust", async () => {
    logger.error(
      "Failed https://example.com/callback?token=abc for admin@example.com with Authorization: Bearer secret-token",
      "/Users/dev/Projects/Web/SiteCMD/.env",
    );

    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
    const [, args] = invokeMock.mock.calls[0] as unknown as [
      string,
      { message: string; context?: string },
    ];

    expect(args.message).toContain("[url]");
    expect(args.message).toContain("[email]");
    expect(args.message).toContain("[secret]");
    expect(args.message).not.toContain("secret-token");
    expect(args.message).not.toContain("admin@example.com");
    expect(args.message).not.toContain("example.com/callback");
    expect(args.context).toBe("[path]");
  });

  it("truncates long frontend log messages before persistence", () => {
    const sanitized = sanitizeFrontendLogText("x".repeat(3_000));

    expect(sanitized).toHaveLength(2_014);
    expect(sanitized.endsWith("...[truncated]")).toBe(true);
  });
});
