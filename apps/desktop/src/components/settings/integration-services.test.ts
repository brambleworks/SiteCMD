import { describe, expect, it } from "vitest";
import { SERVICES, integrationDisplayName } from "@/components/settings/integration-services";

describe("integrationDisplayName", () => {
  it("maps every integration type to its human display name", () => {
    // Toasts and confirmation copy must never show the bare kebab id.
    expect(integrationDisplayName("plausible")).toBe("Plausible Analytics");
    expect(integrationDisplayName("cloudflare")).toBe("Cloudflare");
    expect(integrationDisplayName("uptimerobot")).toBe("UptimeRobot");
    expect(integrationDisplayName("googleanalytics")).toBe("Google Analytics (GA4)");
    expect(integrationDisplayName("googlesearchconsole")).toBe("Google Search Console");
    expect(integrationDisplayName("bingwebmaster")).toBe("Bing Webmaster Tools");
    expect(integrationDisplayName("github")).toBe("GitHub");
    expect(integrationDisplayName("jira")).toBe("Jira");
  });

  it("falls back to the raw type for an unknown id", () => {
    expect(integrationDisplayName("mystery")).toBe("mystery");
  });
});

describe("Bing Webmaster Tools setup copy", () => {
  const bing = SERVICES.find((service) => service.type === "bingwebmaster");

  it("is a configured API-key service", () => {
    expect(bing).toBeDefined();
    expect(bing?.keyLabel).toBe("API Key");
  });

  it("does not link to the nonexistent /apikey page", () => {
    expect(bing?.setupUrl ?? "").not.toMatch(/apikey/i);
    expect(bing?.setupUrl ?? "").toContain("bing.com/webmasters");
  });

  it("directs users to Settings > API Access to find the key", () => {
    const steps = (bing?.setupSteps ?? []).join(" ");
    expect(steps).toMatch(/API Access/i);
    expect(steps).not.toMatch(/Generate API Key/i);
  });
});
