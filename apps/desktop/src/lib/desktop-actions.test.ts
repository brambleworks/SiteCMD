import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: invokeMock,
}));

import {
  extractDesktopCommands,
  isProjectCommandCancelled,
  runProjectCommand,
} from "./desktop-actions";

describe("extractDesktopCommands", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({ success: true, stdout: "", stderr: "", exitCode: 0 });
  });

  it("returns empty array (command extraction disabled for security)", () => {
    expect(
      extractDesktopCommands(`
      1. npm install
      Then run \`pnpm build\`
    `),
    ).toEqual([]);
  });

  it("returns empty array for chained commands", () => {
    expect(extractDesktopCommands("npm install && npm run build")).toEqual([]);
  });

  it("delegates approval and execution to the backend", async () => {
    await runProjectCommand("/tmp/project", "npm install");

    expect(invokeMock).toHaveBeenCalledWith("run_project_command", {
      projectPath: "/tmp/project",
      command: "npm install",
    });
  });

  it("preserves backend cancellation errors", async () => {
    invokeMock.mockRejectedValueOnce("Project command cancelled");

    await expect(runProjectCommand("/tmp/project", "npm install")).rejects.toSatisfy(
      isProjectCommandCancelled,
    );
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});
