import { describe, expect, it } from "vitest";
import { CATEGORY_META } from "./category-meta";
import { CATEGORY_ORDER } from "./tokens";
import type { ScanCategory } from "./types";

describe("CATEGORY_META", () => {
  it("has an entry for every category in CATEGORY_ORDER", () => {
    for (const category of CATEGORY_ORDER) {
      const meta = CATEGORY_META[category];
      expect(meta).toBeDefined();
      expect(meta.label.length).toBeGreaterThan(0);
      expect(meta.shortLabel.length).toBeGreaterThan(0);
      expect(["function", "object"]).toContain(typeof meta.icon);
      expect(meta.accentVar).toMatch(/^--cat-/);
    }
  });

  it("includes the config category even though it's not in CATEGORY_ORDER", () => {
    expect(CATEGORY_META.config).toBeDefined();
    expect(CATEGORY_META.config.label).toBe("Config");
  });

  it("accent vars use bare CSS variable names (no var() wrapper)", () => {
    // Consumers add the `var()` wrapper.
    for (const category of Object.keys(CATEGORY_META) as ScanCategory[]) {
      expect(CATEGORY_META[category].accentVar).not.toMatch(/^var\(/);
    }
  });

  it("compliance is labeled for what it observes, not a legal verdict", () => {
    // The checks under this category disclaim legal conclusions in their own
    // copy (consent_mode.rs, cookie_consent.rs, gdpr.rs, trackers.rs), so the
    // label above them must not promise one either.
    expect(CATEGORY_META.compliance.label).toBe("Privacy & Policies");
    expect(CATEGORY_META.compliance.shortLabel).toBe("Privacy");
    expect(CATEGORY_META.compliance.label).not.toMatch(/legal/i);
    expect(CATEGORY_META.compliance.description).not.toMatch(/legal/i);
  });
});
