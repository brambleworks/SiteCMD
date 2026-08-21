import { describe, expect, it } from "vitest";
import { formatRelativeTime } from "./format";

describe("formatRelativeTime", () => {
  const NOW = new Date("2026-05-08T12:00:00Z").getTime();
  function ago(ms: number) {
    return new Date(NOW - ms);
  }

  it("renders just-now under 60s", () => {
    expect(formatRelativeTime(ago(15_000), NOW)).toBe("just now");
  });
  it("renders minutes", () => {
    expect(formatRelativeTime(ago(5 * 60_000), NOW)).toBe("5m ago");
  });
  it("renders hours", () => {
    expect(formatRelativeTime(ago(3 * 3600_000), NOW)).toBe("3h ago");
  });
  it("renders days", () => {
    expect(formatRelativeTime(ago(2 * 86400_000), NOW)).toBe("2d ago");
  });
  it("accepts ISO string", () => {
    expect(formatRelativeTime(ago(60_000).toISOString(), NOW)).toBe("1m ago");
  });
  it("accepts ms epoch", () => {
    expect(formatRelativeTime(ago(60_000).getTime(), NOW)).toBe("1m ago");
  });

  describe("verbose style", () => {
    it("renders 'yesterday' at 1 day", () => {
      expect(formatRelativeTime(ago(86_400_000), NOW, "verbose")).toBe("yesterday");
    });
    it("renders multi-day 'Xd ago' under a month", () => {
      expect(formatRelativeTime(ago(5 * 86_400_000), NOW, "verbose")).toBe("5d ago");
    });
    it("renders '1 month ago' singular", () => {
      expect(formatRelativeTime(ago(45 * 86_400_000), NOW, "verbose")).toBe("1 month ago");
    });
    it("renders 'X months ago' plural", () => {
      expect(formatRelativeTime(ago(120 * 86_400_000), NOW, "verbose")).toBe("4 months ago");
    });
    it("renders '1 year ago' singular", () => {
      expect(formatRelativeTime(ago(400 * 86_400_000), NOW, "verbose")).toBe("1 year ago");
    });
  });

  describe("compact style at long ranges", () => {
    it("renders months as 'Xmo ago'", () => {
      expect(formatRelativeTime(ago(120 * 86_400_000), NOW)).toBe("4mo ago");
    });
    it("renders years as 'Xy ago'", () => {
      expect(formatRelativeTime(ago(400 * 86_400_000), NOW)).toBe("1y ago");
    });
    it("returns 'unknown' for invalid input", () => {
      expect(formatRelativeTime("not-a-date", NOW)).toBe("unknown");
    });
  });
});
