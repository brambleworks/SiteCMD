import { describe, expect, it } from "vitest";

import type { SiteEvent } from "@/lib/types";

import {
  getAppliedUpdateHistoryRows,
  getUpdateHistoryTitle,
  getVisibleUpdateHistoryEvents,
} from "./update-history";

function makeEvent(overrides: Partial<SiteEvent> = {}): SiteEvent {
  return {
    id: 1,
    projectId: 7,
    eventType: "update",
    severity: "info",
    occurredAtMs: Date.parse("2026-04-12T12:00:00Z"),
    title: "Updates applied",
    summary: "Applied updates",
    detail: null,
    parsedDetail: null,
    source: "internal",
    sourceId: null,
    metadata: null,
    affectedCheckIds: null,
    ...overrides,
  };
}

describe("update history", () => {
  it("ignores invalid persisted counts when building applied update titles", () => {
    expect(
      getUpdateHistoryTitle(
        makeEvent({
          parsedDetail: {
            verified_count: Number.POSITIVE_INFINITY,
            cleared_count: -2,
          },
        }),
        [],
      ),
    ).toBe("1 Update Applied");
  });

  it("rounds finite count values before rendering applied update titles", () => {
    expect(
      getUpdateHistoryTitle(
        makeEvent({
          parsedDetail: {
            verified_count: 2.6,
          },
        }),
        [],
      ),
    ).toBe("3 Updates Applied");
  });

  it("only trusts legacy item labels when the cleared count is a valid single item", () => {
    expect(
      getAppliedUpdateHistoryRows({
        item_label: "react 18.0.0 -> 18.2.0",
        cleared_count: 1,
      }),
    ).toEqual([
      {
        name: "react",
        fromVersion: "18.0.0",
        toVersion: "18.2.0",
      },
    ]);

    expect(
      getAppliedUpdateHistoryRows({
        item_label: "react 18.0.0 -> 18.2.0",
        cleared_count: Number.POSITIVE_INFINITY,
      }),
    ).toEqual([]);
  });

  it("deduplicates equivalent update events after normalizing count fields", () => {
    const detail = {
      applied_updates: [
        {
          name: "react",
          from_version: "18.0.0",
          to_version: "18.2.0",
        },
      ],
      security_updates: 0,
    };
    const visible = getVisibleUpdateHistoryEvents([
      makeEvent({
        id: 2,
        occurredAtMs: Date.parse("2026-04-12T12:05:00Z"),
        parsedDetail: {
          ...detail,
          remaining_updates: 1.2,
        },
      }),
      makeEvent({
        id: 1,
        occurredAtMs: Date.parse("2026-04-12T12:00:00Z"),
        parsedDetail: {
          ...detail,
          remaining_updates: 1.4,
        },
      }),
    ]);

    expect(visible).toHaveLength(1);
    expect(visible[0]?.id).toBe(2);
  });
});
