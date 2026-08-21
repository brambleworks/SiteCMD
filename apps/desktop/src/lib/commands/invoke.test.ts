import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }));

import { command } from "./invoke";

describe("command()", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
  });

  it("calls invoke with only the name when there are no args", async () => {
    await command("pagespeed_api_key_is_set");
    expect(invokeMock).toHaveBeenCalledWith("pagespeed_api_key_is_set");
    expect(invokeMock.mock.calls[0]).toHaveLength(1);
  });

  it("forwards the args object when present", async () => {
    await command("get_scan_execution_detail", { runId: 7 });
    expect(invokeMock).toHaveBeenCalledWith("get_scan_execution_detail", { runId: 7 });
  });
});
