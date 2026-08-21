import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}));
vi.mock("@/lib/tauri-invoke", () => ({ invoke: vi.fn(() => Promise.resolve(null)) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import {
  formatSavedReportDate,
  getHistoryCoverageBadges,
  getReportCoverageBadges,
  isSupportedReportLogo,
  parseHistoryBranding,
  parseHistorySections,
  parseHistorySummary,
  reportFormatLabel,
  toPersistedBranding,
  toReportBrandingPayload,
  type ReportHistorySummary,
  type SectionConfig,
} from "./ReportsPage";
import { loadBranding, loadSections } from "./reports-page-model";

beforeEach(() => {
  localStorage.clear();
});

function sections(overrides: Partial<SectionConfig> = {}): SectionConfig {
  return {
    executive_summary: true,
    category_breakdown: true,
    top_issues: true,
    recommendations: true,
    code_scan: true,
    analytics: true,
    uptime: true,
    deploys: true,
    ...overrides,
  };
}

function summary(overrides: Partial<ReportHistorySummary> = {}): ReportHistorySummary {
  return {
    site_score: 82,
    site_critical: 1,
    site_high: 3,
    site_total: 6,
    latest_scan_date: "2026-04-10T00:00:00Z",
    has_code_scan: true,
    code_critical: 0,
    code_high: 2,
    code_total: 5,
    code_top_domain: "security",
    code_domain_trend: "improved",
    code_checked_at: "2026-04-10T00:00:00Z",
    has_analytics: true,
    has_uptime: true,
    has_deploys: true,
    ...overrides,
  };
}

describe("getReportCoverageBadges", () => {
  it("always includes Web Scan primary badge first", () => {
    const badges = getReportCoverageBadges(sections(), true);
    expect(badges[0]).toEqual({ label: "Web Scan", tone: "primary" });
  });

  it("includes Recommendations when enabled (muted)", () => {
    const badges = getReportCoverageBadges(sections({ recommendations: true }), true);
    expect(badges).toContainEqual({ label: "Recommendations", tone: "muted" });
  });

  it("downgrades Code Scan to warning when no linked folder", () => {
    const badges = getReportCoverageBadges(sections(), false);
    expect(badges).toContainEqual({
      label: "Code Scan needs linked folder",
      tone: "warning",
    });
  });

  it("marks Code Scan as primary when folder is linked", () => {
    const badges = getReportCoverageBadges(sections(), true);
    expect(badges).toContainEqual({ label: "Code Scan", tone: "primary" });
  });

  it("omits Code Scan entirely when section is off", () => {
    const badges = getReportCoverageBadges(sections({ code_scan: false }), true);
    expect(badges.some((b) => b.label.includes("Code Scan"))).toBe(false);
  });
});

describe("getHistoryCoverageBadges", () => {
  it("starts with Web Scan primary", () => {
    const badges = getHistoryCoverageBadges(sections(), summary());
    expect(badges[0]).toEqual({ label: "Web Scan", tone: "primary" });
  });

  it("flags code_scan included vs empty based on summary", () => {
    const withCode = getHistoryCoverageBadges(sections(), summary({ has_code_scan: true }));
    expect(withCode).toContainEqual({ label: "Code Scan included", tone: "primary" });
    const empty = getHistoryCoverageBadges(sections(), summary({ has_code_scan: false }));
    expect(empty).toContainEqual({ label: "Code Scan empty", tone: "warning" });
  });

  it("flags analytics/uptime/deploys missing when summary says so", () => {
    const badges = getHistoryCoverageBadges(
      sections(),
      summary({ has_analytics: false, has_uptime: false, has_deploys: false }),
    );
    expect(badges).toContainEqual({ label: "Analytics missing", tone: "warning" });
    expect(badges).toContainEqual({ label: "Uptime missing", tone: "warning" });
    expect(badges).toContainEqual({ label: "Deployments missing", tone: "warning" });
  });

  it("uses null summary as missing for all gated sections", () => {
    const badges = getHistoryCoverageBadges(sections(), null);
    expect(badges).toContainEqual({ label: "Code Scan empty", tone: "warning" });
    expect(badges).toContainEqual({ label: "Analytics missing", tone: "warning" });
    expect(badges).toContainEqual({ label: "Uptime missing", tone: "warning" });
  });
});

describe("parseHistorySections", () => {
  it("returns default sections when json is null", () => {
    const out = parseHistorySections(null);
    expect(out.executive_summary).toBe(true);
    expect(out.deploys).toBe(true);
  });

  it("merges parsed JSON over defaults", () => {
    const out = parseHistorySections(JSON.stringify({ code_scan: false }));
    expect(out.code_scan).toBe(false);
    expect(out.executive_summary).toBe(true); // default
  });

  it("falls back to defaults on malformed JSON", () => {
    const out = parseHistorySections("not-json-at-all");
    expect(out.executive_summary).toBe(true);
  });

  it("ignores non-boolean persisted section values", () => {
    const out = parseHistorySections(
      JSON.stringify({ code_scan: "false", analytics: false, deploys: 0 }),
    );

    expect(out.code_scan).toBe(true);
    expect(out.analytics).toBe(false);
    expect(out.deploys).toBe(true);
  });
});

describe("parseHistoryBranding", () => {
  const activeBranding = {
    company_name: "Active Agency",
    logo_path: null,
    logo_data_url: "data:image/png;base64,ACTIVE",
    logo_name: "active-logo.png",
    primary_color: "#ff6b00",
    footer_text: "Active footer",
    client_name: "Active Client",
    hide_attribution: false,
  };

  it("falls back to active branding when saved branding JSON is malformed", () => {
    expect(parseHistoryBranding("not-json-at-all", activeBranding)).toEqual(activeBranding);
  });

  it("hydrates a matching saved logo name from active branding", () => {
    const parsed = parseHistoryBranding(
      JSON.stringify({
        company_name: "Saved Agency",
        logo_name: "active-logo.png",
        primary_color: "#2563eb",
        footer_text: "Saved footer",
        hide_attribution: true,
      }),
      activeBranding,
    );

    expect(parsed.company_name).toBe("Saved Agency");
    expect(parsed.logo_data_url).toBe("data:image/png;base64,ACTIVE");
    expect(parsed.hide_attribution).toBe(true);
  });

  it("falls back per field when saved branding has the wrong shape", () => {
    const parsed = parseHistoryBranding(
      JSON.stringify({
        company_name: 123,
        primary_color: "#2563eb",
        footer_text: false,
        hide_attribution: "true",
      }),
      activeBranding,
    );

    expect(parsed.company_name).toBe("SiteCMD");
    expect(parsed.primary_color).toBe("#2563eb");
    expect(parsed.footer_text).toBe("Confidential");
    expect(parsed.hide_attribution).toBe(false);
  });
});

describe("stored report preferences", () => {
  it("sanitizes persisted branding instead of trusting localStorage shape", () => {
    localStorage.setItem(
      "sitecmd_report_branding_7",
      JSON.stringify({
        company_name: ["bad"],
        primary_color: "#2563eb",
        footer_text: 404,
        hide_attribution: "true",
        logo_data_url: "data:image/png;base64,SHOULD_NOT_PERSIST",
      }),
    );

    const loaded = loadBranding(7);

    expect(loaded).toMatchObject({
      company_name: "SiteCMD",
      primary_color: "#2563eb",
      footer_text: "Confidential",
      hide_attribution: false,
      logo_data_url: null,
      logo_path: null,
    });
    expect(JSON.parse(localStorage.getItem("sitecmd_report_branding_7") ?? "{}")).toMatchObject({
      logo_data_url: null,
      logo_path: null,
    });
  });

  it("sanitizes persisted sections instead of trusting localStorage shape", () => {
    localStorage.setItem(
      "sitecmd_report_sections_7",
      JSON.stringify({ code_scan: "false", analytics: false, uptime: null }),
    );

    expect(loadSections(7)).toMatchObject({
      code_scan: true,
      analytics: false,
      uptime: true,
    });
  });
});

describe("parseHistorySummary", () => {
  it("returns null when json is null", () => {
    expect(parseHistorySummary(null)).toBeNull();
  });

  it("returns null on malformed JSON", () => {
    expect(parseHistorySummary("not-json")).toBeNull();
  });

  it("round-trips valid JSON to an object", () => {
    const s = summary({ site_score: 42 });
    const out = parseHistorySummary(JSON.stringify(s));
    expect(out?.site_score).toBe(42);
  });

  it("normalizes legacy web/code saved summaries", () => {
    const out = parseHistorySummary(
      JSON.stringify({
        web_scan_score: 58,
        web_scan_critical: 2,
        web_scan_high: 4,
        web_scan_total: 9,
        code_score: 72,
        has_analytics: true,
        has_uptime: false,
        has_deploys: true,
      }),
    );
    expect(out?.site_score).toBe(58);
    expect(out?.site_critical).toBe(2);
    expect(out?.site_high).toBe(4);
    expect(out?.site_total).toBe(9);
    expect(out?.has_code_scan).toBe(true);
  });
});

describe("formatSavedReportDate", () => {
  it("returns null for null input", () => {
    expect(formatSavedReportDate(null)).toBeNull();
  });

  it("returns the raw string when not a valid date", () => {
    expect(formatSavedReportDate("not a date")).toBe("not a date");
  });

  it("formats valid ISO strings to a locale-formatted date string", () => {
    // Locale output varies, just assert the month abbreviation is present.
    const out = formatSavedReportDate("2026-04-10T00:00:00Z");
    expect(out).not.toBeNull();
    expect(out).toMatch(/Apr|April|4/);
  });
});

describe("reportFormatLabel", () => {
  it("returns 'PDF' for 'pdf'", () => {
    expect(reportFormatLabel("pdf")).toBe("PDF");
  });
  it("returns 'HTML' for 'html'", () => {
    expect(reportFormatLabel("html")).toBe("HTML");
  });
  it("returns 'Preview' for unknown formats", () => {
    expect(reportFormatLabel("")).toBe("Preview");
    expect(reportFormatLabel("docx")).toBe("Preview");
  });
});

describe("report logo helpers", () => {
  it("accepts supported raster logo types and rejects unsupported ones", () => {
    expect(isSupportedReportLogo({ type: "image/png", name: "logo.png" } as File)).toBe(true);
    expect(isSupportedReportLogo({ type: "image/svg+xml", name: "logo.svg" } as File)).toBe(false);
    expect(isSupportedReportLogo({ type: "", name: "logo.webp" } as File)).toBe(true);
  });

  it("strips preview-only logo paths from the backend branding payload", () => {
    expect(
      toReportBrandingPayload({
        company_name: "SiteCMD",
        logo_path: "/Users/dev/Pictures/logo.png",
        logo_data_url: "data:image/png;base64,AAAA",
        logo_name: "logo.png",
        primary_color: "#2563eb",
        footer_text: "Confidential",
        client_name: null,
        hide_attribution: false,
      }),
    ).toEqual({
      companyName: "SiteCMD",
      logoPath: null,
      logoDataUrl: "data:image/png;base64,AAAA",
      logoName: "logo.png",
      primaryColor: "#2563eb",
      footerText: "Confidential",
      clientName: null,
      hideAttribution: false,
    });
  });

  it("strips inline logo payloads from persisted branding settings", () => {
    expect(
      toPersistedBranding({
        company_name: "SiteCMD",
        logo_path: "/Users/dev/Pictures/logo.png",
        logo_data_url: "data:image/png;base64,AAAA",
        logo_name: "logo.png",
        primary_color: "#2563eb",
        footer_text: "Confidential",
        client_name: null,
        hide_attribution: false,
      }),
    ).toEqual({
      company_name: "SiteCMD",
      logo_path: null,
      logo_data_url: null,
      logo_name: "logo.png",
      primary_color: "#2563eb",
      footer_text: "Confidential",
      client_name: null,
      hide_attribution: false,
    });
  });
});
