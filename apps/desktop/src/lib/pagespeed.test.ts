import { describe, expect, it } from "vitest";
import { rateVital, ratePerformanceScore, ratingColorClass } from "./pagespeed";

describe("rateVital", () => {
  it("rates LCP on Google thresholds (<=2.5s good, <=4s needs, else poor)", () => {
    expect(rateVital("lcp", 2000)).toBe("good");
    expect(rateVital("lcp", 2500)).toBe("good");
    expect(rateVital("lcp", 3000)).toBe("needs-improvement");
    expect(rateVital("lcp", 4500)).toBe("poor");
  });

  it("rates CLS (<=0.1 good, <=0.25 needs, else poor)", () => {
    expect(rateVital("cls", 0.1)).toBe("good");
    expect(rateVital("cls", 0.2)).toBe("needs-improvement");
    expect(rateVital("cls", 0.3)).toBe("poor");
  });

  it("rates INP and TBT", () => {
    expect(rateVital("inp", 200)).toBe("good");
    expect(rateVital("inp", 400)).toBe("needs-improvement");
    expect(rateVital("inp", 600)).toBe("poor");
    expect(rateVital("tbt", 200)).toBe("good");
    expect(rateVital("tbt", 700)).toBe("poor");
  });

  it("returns null when the value is missing", () => {
    expect(rateVital("lcp", null)).toBeNull();
    expect(rateVital("cls", null)).toBeNull();
  });
});

describe("ratePerformanceScore", () => {
  it("uses Lighthouse bands (>=90 good, >=50 needs, else poor)", () => {
    expect(ratePerformanceScore(100)).toBe("good");
    expect(ratePerformanceScore(90)).toBe("good");
    expect(ratePerformanceScore(75)).toBe("needs-improvement");
    expect(ratePerformanceScore(50)).toBe("needs-improvement");
    expect(ratePerformanceScore(30)).toBe("poor");
  });
});

describe("ratingColorClass", () => {
  it("maps each rating to a theme token class", () => {
    expect(ratingColorClass("good")).toBe("text-score-excellent");
    expect(ratingColorClass("needs-improvement")).toBe("text-severity-high");
    expect(ratingColorClass("poor")).toBe("text-severity-critical");
    expect(ratingColorClass(null)).toBe("text-muted-foreground");
  });
});
