import { describe, expect, it } from "vitest";
import {
  addSeverityCounts,
  compareSeverity,
  createSeverityCounts,
  formatSeverityLabel,
  formatSeverityToneClass,
  isSeverity,
  severityCssVar,
  severityCountTotal,
  severityRank,
  severityLabel,
  severityToneClass,
  type Severity,
} from "./severity";

describe("severity helpers", () => {
  it("ranks critical < high < medium < low", () => {
    expect(severityRank("critical")).toBeLessThan(severityRank("high"));
    expect(severityRank("high")).toBeLessThan(severityRank("medium"));
    expect(severityRank("medium")).toBeLessThan(severityRank("low"));
  });
  it("compareSeverity sorts critical first", () => {
    const arr: Severity[] = ["low", "critical", "medium", "high"];
    arr.sort(compareSeverity);
    expect(arr).toEqual(["critical", "high", "medium", "low"]);
  });
  it("provides human label", () => {
    expect(severityLabel("high")).toBe("High");
    expect(severityLabel("critical")).toBe("Critical");
    expect(formatSeverityLabel("medium")).toBe("Medium");
    expect(formatSeverityLabel("warn")).toBe("warn");
  });
  it("provides tone class", () => {
    expect(severityToneClass("critical")).toBe("text-severity-critical");
    expect(severityToneClass("high")).toBe("text-severity-high");
    expect(severityToneClass("medium")).toBe("text-severity-medium");
    expect(severityToneClass("low")).toBe("text-severity-low");
    expect(severityCssVar("low")).toBe("var(--severity-low)");
  });
  it("maps untyped severity strings to tone classes with a muted fallback", () => {
    expect(formatSeverityToneClass("critical")).toBe("text-severity-critical");
    expect(formatSeverityToneClass("warn")).toBe("text-muted-foreground");
    expect(formatSeverityToneClass("")).toBe("text-muted-foreground");
  });
  it("creates, adds, and totals severity count records", () => {
    expect(createSeverityCounts({ high: 2 })).toEqual({
      critical: 0,
      high: 2,
      medium: 0,
      low: 0,
    });
    expect(
      addSeverityCounts(createSeverityCounts({ critical: 1 }), createSeverityCounts({ low: 3 })),
    ).toEqual({ critical: 1, high: 0, medium: 0, low: 3 });
    expect(severityCountTotal(createSeverityCounts({ critical: 1, high: 2, low: 3 }))).toBe(6);
    expect(severityCountTotal(createSeverityCounts({ critical: 1, high: 2 }), ["critical"])).toBe(
      1,
    );
  });
  it("detects valid issue severities", () => {
    expect(isSeverity("critical")).toBe(true);
    expect(isSeverity("warning")).toBe(false);
    expect(isSeverity(null)).toBe(false);
  });
});
