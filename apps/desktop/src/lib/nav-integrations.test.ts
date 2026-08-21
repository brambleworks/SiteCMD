import { describe, expect, it } from "vitest";
import {
  isNavPageConnected,
  isProgressiveNavPage,
  PROGRESSIVE_NAV_INTEGRATIONS,
  PROGRESSIVE_NAV_PAGES,
} from "./nav-integrations";

describe("nav-integrations", () => {
  it("pins which integration-fed pages are progressive", () => {
    expect(PROGRESSIVE_NAV_PAGES).toEqual(["analytics", "search-console", "deploys"]);
  });

  it("pins the sources that feed each page, matching the Dashboard signal cards", () => {
    expect(PROGRESSIVE_NAV_INTEGRATIONS.analytics).toEqual([
      "plausible",
      "googleanalytics",
      "cloudflare",
    ]);
    expect(PROGRESSIVE_NAV_INTEGRATIONS["search-console"]).toEqual([
      "googlesearchconsole",
      "bingwebmaster",
    ]);
    expect(PROGRESSIVE_NAV_INTEGRATIONS.deploys).toEqual(["github"]);
  });

  it("treats a page as connected when any of its sources is enabled", () => {
    expect(isNavPageConnected("analytics", new Set(["plausible"]))).toBe(true);
    expect(isNavPageConnected("analytics", new Set(["cloudflare"]))).toBe(true);
    expect(isNavPageConnected("search-console", new Set(["bingwebmaster"]))).toBe(true);
    expect(isNavPageConnected("deploys", new Set(["github"]))).toBe(true);
  });

  it("treats a page as disconnected when only unrelated sources are enabled", () => {
    // UptimeRobot feeds no progressive page, so it must not surface any of them.
    expect(isNavPageConnected("analytics", new Set(["uptimerobot"]))).toBe(false);
    expect(isNavPageConnected("deploys", new Set(["plausible"]))).toBe(false);
    expect(isNavPageConnected("search-console", new Set())).toBe(false);
  });

  it("recognizes only the progressive pages", () => {
    expect(isProgressiveNavPage("analytics")).toBe(true);
    expect(isProgressiveNavPage("deploys")).toBe(true);
    expect(isProgressiveNavPage("dashboard")).toBe(false);
    expect(isProgressiveNavPage("issues")).toBe(false);
  });
});
