import { beforeEach, describe, expect, it, vi } from "vitest";

import { loadCodeFixGuide, loadWebFixGuide } from "./async-fix-guides";

const resolveFixGuide = vi.hoisted(() => vi.fn());
vi.mock("./commands/catalog", () => ({ resolveFixGuide }));

beforeEach(() => {
  resolveFixGuide.mockReset();
});

describe("loadWebFixGuide", () => {
  it("sends the check id, ordered variant candidates, and the bundled steps", async () => {
    resolveFixGuide.mockResolvedValue({ steps: ["catalog step"], source: "catalog" });

    await loadWebFixGuide("security.csp", {
      cdn: "Cloudflare",
      cms: "WordPress",
      framework: "Next",
    });

    expect(resolveFixGuide).toHaveBeenCalledWith(
      expect.objectContaining({
        checkId: "security.csp",
        variantCandidates: ["next", "wordpress", "cloudflare"],
      }),
    );
  });

  it("offers both bare and .js-suffixed variant keys for real detector names", async () => {
    resolveFixGuide.mockResolvedValue({ steps: ["catalog step"], source: "catalog" });

    await loadWebFixGuide("security.csp", { framework: "Next.js" });
    expect(resolveFixGuide).toHaveBeenLastCalledWith(
      expect.objectContaining({ variantCandidates: ["next", "next.js"] }),
    );

    await loadWebFixGuide("security.csp", { framework: "Nuxt.js", cdn: "Vercel" });
    expect(resolveFixGuide).toHaveBeenLastCalledWith(
      expect.objectContaining({ variantCandidates: ["nuxt", "nuxt.js", "vercel"] }),
    );

    await loadCodeFixGuide("csrf-missing", "Next.js");
    expect(resolveFixGuide).toHaveBeenLastCalledWith(
      expect.objectContaining({ variantCandidates: ["next", "next.js"] }),
    );
  });

  it("canonicalizes aliased and polish ids before the catalog lookup", async () => {
    resolveFixGuide.mockResolvedValue({ steps: ["catalog step"], source: "catalog" });

    await loadWebFixGuide("security.headers.csp", null);
    expect(resolveFixGuide).toHaveBeenLastCalledWith(
      expect.objectContaining({ checkId: "security.csp" }),
    );

    await loadWebFixGuide("polish.em-dash-density", null);
    expect(resolveFixGuide).toHaveBeenLastCalledWith(
      expect.objectContaining({ checkId: "em-dash-density" }),
    );
  });

  it("omits stack values that are not strings", async () => {
    resolveFixGuide.mockResolvedValue(null);
    await loadWebFixGuide("security.csp", { cms: null, framework: 42 });
    expect(resolveFixGuide).toHaveBeenCalledWith(
      expect.objectContaining({ variantCandidates: [] }),
    );
  });

  it("falls back to the bundled effort when the catalog entry carries none", async () => {
    resolveFixGuide.mockResolvedValue({
      catalogVersion: "2026.07.26",
      source: "catalog",
      steps: ["catalog step"],
    });

    const guide = await loadWebFixGuide("security.csp", null);
    expect(guide?.steps).toEqual(["catalog step"]);
    expect(guide?.effortMinutes).toBeGreaterThan(0);
  });

  it("prefers the catalog's own effort over the baseline's when both exist", async () => {
    resolveFixGuide.mockResolvedValue({
      catalogVersion: "2026.07.26",
      effort: "involved",
      effortMinutes: 60,
      source: "catalog",
      steps: ["deep catalog step"],
    });

    const guide = await loadWebFixGuide("security.csp", null);
    expect(guide?.steps).toEqual(["deep catalog step"]);
    expect(guide?.effort).toBe("involved");
    expect(guide?.effortMinutes).toBe(60);
  });

  it("keeps the bundled guidance when the backend call fails", async () => {
    resolveFixGuide.mockRejectedValue(new Error("ipc unavailable"));

    const guide = await loadWebFixGuide("security.csp", null);
    expect(guide).not.toBeNull();
    expect(guide!.steps.length).toBeGreaterThan(0);
  });

  it("returns null for a check neither source has", async () => {
    resolveFixGuide.mockResolvedValue(null);
    expect(await loadWebFixGuide("not.a.real.check", null)).toBeNull();
  });
});

describe("loadCodeFixGuide", () => {
  it("passes a lowercased framework as the only candidate", async () => {
    resolveFixGuide.mockResolvedValue({ steps: ["catalog step"], source: "catalog" });
    await loadCodeFixGuide("csrf-missing", "Django");
    expect(resolveFixGuide).toHaveBeenCalledWith(
      expect.objectContaining({ variantCandidates: ["django"] }),
    );
  });

  it("keeps the bundled guidance when the backend call fails", async () => {
    resolveFixGuide.mockRejectedValue(new Error("ipc unavailable"));
    const guide = await loadCodeFixGuide("csrf-missing", null);
    if (guide) expect(guide.steps.length).toBeGreaterThan(0);
  });
});
