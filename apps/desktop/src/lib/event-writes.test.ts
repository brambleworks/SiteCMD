import { beforeEach, describe, expect, it, vi } from "vitest";

const { emitAppEventMock, recordSearchEventCmd, recordUpdateEventCmd } = vi.hoisted(() => ({
  emitAppEventMock: vi.fn(),
  recordSearchEventCmd: vi.fn(async () => 11),
  recordUpdateEventCmd: vi.fn(async () => 22),
}));

vi.mock("@/lib/app-events", () => ({ emitAppEvent: emitAppEventMock }));
vi.mock("@/lib/commands", () => ({
  recordSearchEvent: recordSearchEventCmd,
  recordUpdateEvent: recordUpdateEventCmd,
}));

import { publishEventsRecorded, recordSearchEvent, recordUpdateEvent } from "./event-writes";

const args = { projectId: 4, title: "Search issue verified", summary: "Fixed" };

describe("event-writes", () => {
  beforeEach(() => {
    emitAppEventMock.mockClear();
    recordSearchEventCmd.mockClear();
    recordUpdateEventCmd.mockClear();
  });

  it("announces every recorded search event", async () => {
    await expect(recordSearchEvent(args)).resolves.toBe(11);
    expect(recordSearchEventCmd).toHaveBeenCalledWith(args);
    expect(emitAppEventMock).toHaveBeenCalledWith("events-recorded", { projectId: 4 });
  });

  it("announces every recorded update event", async () => {
    await expect(recordUpdateEvent(args)).resolves.toBe(22);
    expect(emitAppEventMock).toHaveBeenCalledWith("events-recorded", { projectId: 4 });
  });

  it("does not announce when the write itself failed", async () => {
    recordSearchEventCmd.mockRejectedValueOnce(new Error("db locked"));
    await expect(recordSearchEvent(args)).rejects.toThrow("db locked");
    expect(emitAppEventMock).not.toHaveBeenCalled();
  });

  it("exposes a standalone announcement for queued poll writers", () => {
    publishEventsRecorded(9);
    expect(emitAppEventMock).toHaveBeenCalledWith("events-recorded", { projectId: 9 });
  });
});
