import { describe, expect, it } from "vitest";

import {
  getSeoFocusLabel,
  getSeoWatchImpactSentence,
  inferSeoFocusFromText,
  matchesSeoFocusText,
} from "./seo-focus";

describe("seo-focus", () => {
  it("returns labels for known focuses", () => {
    expect(getSeoFocusLabel("seo.robots")).toBe("robots.txt");
    expect(getSeoFocusLabel("seo.structured_data")).toBe("structured data");
    expect(getSeoFocusLabel("unknown.focus")).toBeNull();
  });

  it("matches known focus patterns against issue text", () => {
    expect(matchesSeoFocusText("seo.robots_txt Robots blocked", "seo.robots")).toBe(true);
    expect(matchesSeoFocusText("seo.canonical Missing canonical tag", "seo.robots")).toBe(false);
  });

  it("infers focus using the shared matcher order", () => {
    expect(inferSeoFocusFromText("seo.meta_description Missing meta description")).toBe(
      "seo.descriptions",
    );
    expect(inferSeoFocusFromText("seo.canonical Missing canonical tag")).toBe("seo.canonical");
  });

  it("returns shared watched-file impact copy", () => {
    expect(getSeoWatchImpactSentence()).toContain("crawl directives");
  });
});
