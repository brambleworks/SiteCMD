import { describe, expect, it } from "vitest";
import {
  CATEGORY_CSS_VAR,
  CATEGORY_LABELS,
  CATEGORY_ORDER,
  CATEGORY_TEXT,
  SCORE_CSS_VAR,
  formatBytes,
  formatCheckName,
  formatDate,
  formatDuration,
  formatNum,
  formatRelativeTime,
  getScoreCssVar,
} from "./tokens";
import { CATEGORY_META } from "./category-meta";

describe("CATEGORY_ORDER", () => {
  it("category order contains all seven categories", () => {
    expect(CATEGORY_ORDER).toEqual([
      "security",
      "performance",
      "seo",
      "accessibility",
      "compliance",
      "config",
      "polish",
    ]);
  });

  it("category labels mirror CATEGORY_META, including unweighted config", () => {
    for (const [category, meta] of Object.entries(CATEGORY_META)) {
      expect(CATEGORY_LABELS[category as keyof typeof CATEGORY_LABELS]).toBe(meta.label);
      expect(CATEGORY_TEXT[category as keyof typeof CATEGORY_TEXT]).toContain(`cat-${category}`);
      expect(CATEGORY_CSS_VAR[category as keyof typeof CATEGORY_CSS_VAR]).toBe(
        `var(--cat-${category})`,
      );
    }
  });
});

describe("getScoreCssVar", () => {
  it("returns excellent for >= 90", () => {
    expect(getScoreCssVar(95)).toBe(SCORE_CSS_VAR.excellent);
    expect(getScoreCssVar(90)).toBe(SCORE_CSS_VAR.excellent);
  });

  it("returns good for 70..89", () => {
    expect(getScoreCssVar(89)).toBe(SCORE_CSS_VAR.good);
    expect(getScoreCssVar(70)).toBe(SCORE_CSS_VAR.good);
  });

  it("returns attention for 50..69", () => {
    expect(getScoreCssVar(69)).toBe(SCORE_CSS_VAR.attention);
    expect(getScoreCssVar(50)).toBe(SCORE_CSS_VAR.attention);
  });

  it("returns poor for 30..49", () => {
    expect(getScoreCssVar(49)).toBe(SCORE_CSS_VAR.poor);
    expect(getScoreCssVar(30)).toBe(SCORE_CSS_VAR.poor);
  });

  it("returns critical for < 30", () => {
    expect(getScoreCssVar(29)).toBe(SCORE_CSS_VAR.critical);
    expect(getScoreCssVar(0)).toBe(SCORE_CSS_VAR.critical);
  });
});

describe("formatNum", () => {
  it("returns '0' for null/undefined", () => {
    expect(formatNum(null)).toBe("0");
    expect(formatNum(undefined)).toBe("0");
  });

  it("formats < 1000 as plain string", () => {
    expect(formatNum(0)).toBe("0");
    expect(formatNum(999)).toBe("999");
  });

  it("formats 1K..999K with K suffix and one decimal", () => {
    expect(formatNum(1_000)).toBe("1.0K");
    expect(formatNum(12_345)).toBe("12.3K");
  });

  it("formats 1M+ with M suffix and one decimal", () => {
    expect(formatNum(1_500_000)).toBe("1.5M");
  });
});

describe("formatBytes", () => {
  it("returns '0 B' for null/undefined/zero", () => {
    expect(formatBytes(null)).toBe("0 B");
    expect(formatBytes(undefined)).toBe("0 B");
    expect(formatBytes(0)).toBe("0 B");
  });

  it("formats bytes < 1024 as 'N B'", () => {
    expect(formatBytes(512)).toBe("512 B");
  });

  it("formats KB/MB/GB", () => {
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(1_500_000)).toBe("1.4 MB");
    expect(formatBytes(2_147_483_648)).toBe("2.0 GB");
  });
});

describe("formatRelativeTime", () => {
  it("reports 'just now' within the first minute", () => {
    const nowMs = new Date("2026-04-10T12:00:30Z").getTime();
    const earlier = new Date("2026-04-10T12:00:00Z");
    expect(formatRelativeTime(earlier, nowMs)).toBe("just now");
  });

  it("reports minutes for < 1h", () => {
    const nowMs = new Date("2026-04-10T12:30:00Z").getTime();
    expect(formatRelativeTime(new Date("2026-04-10T12:15:00Z"), nowMs)).toBe("15m ago");
  });

  it("reports hours for < 24h", () => {
    const nowMs = new Date("2026-04-10T12:00:00Z").getTime();
    expect(formatRelativeTime(new Date("2026-04-10T09:00:00Z"), nowMs)).toBe("3h ago");
  });

  it("reports days for < 7d", () => {
    const nowMs = new Date("2026-04-10T12:00:00Z").getTime();
    expect(formatRelativeTime(new Date("2026-04-07T12:00:00Z"), nowMs)).toBe("3d ago");
  });
});

describe("formatDuration", () => {
  it("returns '0s' for null/undefined/zero", () => {
    expect(formatDuration(null)).toBe("0s");
    expect(formatDuration(undefined)).toBe("0s");
    expect(formatDuration(0)).toBe("0s");
  });

  it("formats < 60s as seconds", () => {
    expect(formatDuration(5)).toBe("5s");
    expect(formatDuration(59)).toBe("59s");
  });

  it("formats minutes with optional seconds", () => {
    expect(formatDuration(60)).toBe("1m");
    expect(formatDuration(90)).toBe("1m 30s");
  });

  it("formats hours with optional minutes", () => {
    expect(formatDuration(3600)).toBe("1h");
    expect(formatDuration(3600 + 15 * 60)).toBe("1h 15m");
  });
});

describe("formatDate", () => {
  it("renders month/day + 'at' + time", () => {
    // Month and time formatting is locale-dependent; just assert structure
    const out = formatDate("2026-04-10T12:00:00Z");
    expect(out).toMatch(/at/);
  });
});

describe("formatCheckName", () => {
  it("converts snake_case / kebab-case ids to Title Case", () => {
    expect(formatCheckName("robots_txt_missing")).toBe("Robots Txt Missing");
    expect(formatCheckName("broken-link")).toBe("Broken Link");
    expect(formatCheckName("security.ssl")).toBe("Security SSL");
  });

  it("uppercases known acronyms", () => {
    expect(formatCheckName("ssl_expiring")).toBe("SSL Expiring");
    expect(formatCheckName("https_upgrade")).toBe("HTTPS Upgrade");
    expect(formatCheckName("seo_title")).toBe("SEO Title");
    expect(formatCheckName("hsts_missing")).toBe("HSTS Missing");
    expect(formatCheckName("csp_weak")).toBe("CSP Weak");
    expect(formatCheckName("dns_soa")).toBe("DNS Soa");
    expect(formatCheckName("wcag_contrast")).toBe("WCAG Contrast");
  });

  it("handles 'http_' prefix specifically", () => {
    expect(formatCheckName("http_redirect")).toBe("HTTP Redirect");
  });
});
