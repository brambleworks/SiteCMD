import { describe, expect, it } from "vitest";

import {
  matchesProjectSignalsChangedEvent,
  type ProjectSignalsChangedEvent,
} from "./project-signal-events";

describe("matchesProjectSignalsChangedEvent", () => {
  const payload: ProjectSignalsChangedEvent = {
    projectId: 7,
    url: "https://example.com/",
    source: "desktop-watch",
  };

  it("matches the same project even when trailing slashes differ", () => {
    expect(
      matchesProjectSignalsChangedEvent(payload, {
        projectId: 7,
        url: "https://example.com",
      }),
    ).toBe(true);
  });

  it("does not match a different project", () => {
    expect(
      matchesProjectSignalsChangedEvent(payload, {
        projectId: 8,
        url: "https://example.com",
      }),
    ).toBe(false);
  });

  it("treats missing URLs as project-scoped refreshes", () => {
    expect(
      matchesProjectSignalsChangedEvent(
        {
          ...payload,
          url: null,
        },
        {
          projectId: 7,
          url: "https://example.com",
        },
      ),
    ).toBe(true);
  });

  it("does not match a different site URL on the same project when both URLs exist", () => {
    expect(
      matchesProjectSignalsChangedEvent(payload, {
        projectId: 7,
        url: "https://staging.example.com",
      }),
    ).toBe(false);
  });
});
