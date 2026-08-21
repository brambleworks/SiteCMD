import { convertFileSrc } from "@tauri-apps/api/core";
import { Database, Eye, FileCode, FileText, LayoutGrid, Search, Shield, Zap } from "lucide-react";
import { parseJsonRecord, type JsonRecord } from "@/lib/json-record";

export type ReportData = import("./ReportPDF").ReportData;

export const PERIOD_OPTIONS = [
  { label: "Last 7 Days", value: 7 },
  { label: "Last 30 Days", value: 30 },
  { label: "Last 90 Days", value: 90 },
];

export interface Branding {
  company_name: string;
  logo_path: string | null;
  logo_data_url?: string | null;
  logo_name?: string | null;
  primary_color: string;
  footer_text: string;
  client_name: string | null;
  hide_attribution: boolean;
}

export interface SectionConfig {
  executive_summary: boolean;
  category_breakdown: boolean;
  top_issues: boolean;
  recommendations: boolean;
  code_scan: boolean;
  analytics: boolean;
  uptime: boolean;
  deploys: boolean;
}

export const SECTION_LABELS: { key: keyof SectionConfig; label: string; icon: typeof FileText }[] =
  [
    { key: "executive_summary", label: "Executive Summary", icon: FileText },
    { key: "category_breakdown", label: "Web Scan Breakdown", icon: Shield },
    { key: "top_issues", label: "Top Issues", icon: Zap },
    { key: "recommendations", label: "Recommendations", icon: Search },
    { key: "code_scan", label: "Code Scan", icon: FileCode },
    { key: "analytics", label: "Analytics", icon: Eye },
    { key: "uptime", label: "Uptime", icon: LayoutGrid },
    { key: "deploys", label: "Deployments", icon: Database },
  ];

// Per-project localStorage keys. Old global keys are migrated on first load.
const OLD_BRANDING_KEY = "sitehealthkit_report_branding";
const OLD_SECTIONS_KEY = "sitehealthkit_report_sections";

function brandingKey(projectId: number | null) {
  return `sitecmd_report_branding_${projectId ?? 0}`;
}

function sectionsKey(projectId: number | null) {
  return `sitecmd_report_sections_${projectId ?? 0}`;
}

const DEFAULT_BRANDING: Branding = {
  company_name: "SiteCMD",
  logo_path: null,
  logo_data_url: null,
  logo_name: null,
  primary_color: "var(--primary)",
  footer_text: "Confidential",
  client_name: null,
  hide_attribution: false,
};

const DEFAULT_SECTIONS: SectionConfig = {
  executive_summary: true,
  category_breakdown: true,
  top_issues: true,
  recommendations: true,
  code_scan: true,
  analytics: true,
  uptime: true,
  deploys: true,
};

export const REPORT_LOGO_ACCEPT = "image/png,image/jpeg,image/webp,image/gif";
export const REPORT_LOGO_MAX_BYTES = 2 * 1024 * 1024;

export function isSupportedReportLogo(file: Pick<File, "type" | "name">): boolean {
  if (file.type && ["image/png", "image/jpeg", "image/webp", "image/gif"].includes(file.type)) {
    return true;
  }
  return /\.(png|jpe?g|webp|gif)$/i.test(file.name);
}

export function toReportBrandingPayload(branding: Branding): ReportData["branding"] {
  return {
    companyName: branding.company_name,
    logoPath: null,
    logoDataUrl: branding.logo_data_url ?? null,
    logoName: branding.logo_name ?? null,
    primaryColor: branding.primary_color,
    footerText: branding.footer_text,
    clientName: branding.client_name,
    hideAttribution: branding.hide_attribution,
  };
}

function toSectionConfigPayload(sections: SectionConfig): ReportData["sections"] {
  return {
    executiveSummary: sections.executive_summary,
    categoryBreakdown: sections.category_breakdown,
    topIssues: sections.top_issues,
    recommendations: sections.recommendations,
    codeScan: sections.code_scan,
    analytics: sections.analytics,
    uptime: sections.uptime,
    deploys: sections.deploys,
  };
}

export function toPersistedBranding(branding: Branding): Branding {
  return {
    ...branding,
    logo_path: null,
    logo_data_url: null,
  };
}

export function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result === "string") {
        resolve(result);
      } else {
        reject(new Error("Failed to load logo image"));
      }
    };
    reader.onerror = () => reject(reader.error ?? new Error("Failed to load logo image"));
    reader.readAsDataURL(file);
  });
}

export function getBrandingPreviewSrc(branding: Branding): string | null {
  if (branding.logo_data_url) return branding.logo_data_url;
  if (branding.logo_path) return convertFileSrc(branding.logo_path);
  return null;
}

function hydrateHistoryBranding(savedBranding: Branding, activeBranding: Branding): Branding {
  if (
    !savedBranding.logo_data_url &&
    !savedBranding.logo_path &&
    savedBranding.logo_name &&
    activeBranding.logo_name === savedBranding.logo_name &&
    activeBranding.logo_data_url
  ) {
    return {
      ...savedBranding,
      logo_data_url: activeBranding.logo_data_url,
    };
  }
  return savedBranding;
}

function stringField(value: unknown, fallback: string): string {
  return typeof value === "string" ? value : fallback;
}

function nullableStringField(value: unknown, fallback: string | null): string | null {
  if (typeof value === "string") return value;
  if (value === null) return null;
  return fallback;
}

function booleanField(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function brandingFromRecord(record: JsonRecord, fallback: Branding = DEFAULT_BRANDING): Branding {
  return {
    company_name: stringField(record.company_name, fallback.company_name),
    logo_path: nullableStringField(record.logo_path, fallback.logo_path),
    logo_data_url: nullableStringField(record.logo_data_url, fallback.logo_data_url ?? null),
    logo_name: nullableStringField(record.logo_name, fallback.logo_name ?? null),
    primary_color: stringField(record.primary_color, fallback.primary_color),
    footer_text: stringField(record.footer_text, fallback.footer_text),
    client_name: nullableStringField(record.client_name, fallback.client_name),
    hide_attribution: booleanField(record.hide_attribution, fallback.hide_attribution),
  };
}

function sectionsFromRecord(record: JsonRecord): SectionConfig {
  return {
    executive_summary: booleanField(record.executive_summary, DEFAULT_SECTIONS.executive_summary),
    category_breakdown: booleanField(
      record.category_breakdown,
      DEFAULT_SECTIONS.category_breakdown,
    ),
    top_issues: booleanField(record.top_issues, DEFAULT_SECTIONS.top_issues),
    recommendations: booleanField(record.recommendations, DEFAULT_SECTIONS.recommendations),
    code_scan: booleanField(record.code_scan, DEFAULT_SECTIONS.code_scan),
    analytics: booleanField(record.analytics, DEFAULT_SECTIONS.analytics),
    uptime: booleanField(record.uptime, DEFAULT_SECTIONS.uptime),
    deploys: booleanField(record.deploys, DEFAULT_SECTIONS.deploys),
  };
}

export function parseHistoryBranding(
  brandingJson: string | null,
  activeBranding: Branding,
): Branding {
  if (!brandingJson) return activeBranding;
  const parsed = parseJsonRecord(brandingJson);
  if (!parsed) return activeBranding;
  return hydrateHistoryBranding(brandingFromRecord(parsed), activeBranding);
}

/** Migrate old global keys to current project if new per-project keys don't exist yet. */
function migrateReportKeys(projectId: number | null) {
  try {
    const key = brandingKey(projectId);
    if (!localStorage.getItem(key)) {
      const old = localStorage.getItem(OLD_BRANDING_KEY);
      if (old) {
        const parsed = parseJsonRecord(old);
        localStorage.setItem(
          key,
          parsed ? JSON.stringify(toPersistedBranding(brandingFromRecord(parsed))) : old,
        );
        localStorage.removeItem(OLD_BRANDING_KEY);
      }
    }
    const sKey = sectionsKey(projectId);
    if (!localStorage.getItem(sKey)) {
      const old = localStorage.getItem(OLD_SECTIONS_KEY);
      if (old) {
        localStorage.setItem(sKey, old);
        localStorage.removeItem(OLD_SECTIONS_KEY);
      }
    }
  } catch {
    // Best-effort migration.
  }
}

export function loadBranding(projectId: number | null): Branding {
  migrateReportKeys(projectId);
  try {
    const saved = localStorage.getItem(brandingKey(projectId));
    if (saved) {
      const parsed = parseJsonRecord(saved);
      if (!parsed) return { ...DEFAULT_BRANDING };
      const sanitized = toPersistedBranding(brandingFromRecord(parsed));
      if (typeof parsed.logo_path === "string" || typeof parsed.logo_data_url === "string") {
        saveBranding(projectId, sanitized);
      }
      return sanitized;
    }
  } catch {
    // Fall through to default.
  }
  return { ...DEFAULT_BRANDING };
}

export function saveBranding(projectId: number | null, branding: Branding): boolean {
  try {
    localStorage.setItem(brandingKey(projectId), JSON.stringify(toPersistedBranding(branding)));
    return true;
  } catch {
    return false;
  }
}

export function loadSections(projectId: number | null): SectionConfig {
  migrateReportKeys(projectId);
  try {
    const saved = localStorage.getItem(sectionsKey(projectId));
    if (saved) {
      const parsed = parseJsonRecord(saved);
      if (parsed) return sectionsFromRecord(parsed);
    }
  } catch {
    // Fall through to default.
  }
  return { ...DEFAULT_SECTIONS };
}

export function saveSections(projectId: number | null, sections: SectionConfig): boolean {
  try {
    localStorage.setItem(sectionsKey(projectId), JSON.stringify(sections));
    return true;
  } catch {
    return false;
  }
}

export interface ReportHistoryEntry {
  id: number;
  projectId: number;
  siteUrl: string;
  periodDays: number;
  reportTitle: string;
  outputFormat: string;
  generatedAt: string;
  brandingJson: string | null;
  sectionsJson: string | null;
  reportSummaryJson: string | null;
}

export interface ReportHistorySummary {
  site_score: number;
  site_critical: number;
  site_high: number;
  site_total: number;
  latest_scan_date: string | null;
  has_code_scan: boolean;
  code_critical: number | null;
  code_high: number | null;
  code_total: number | null;
  code_top_domain: string | null;
  code_domain_trend: string | null;
  code_checked_at: string | null;
  has_analytics: boolean;
  has_uptime: boolean;
  has_deploys: boolean;
}

export function applyReportPresentation(
  data: ReportData,
  overrides?: {
    branding?: Branding;
    reportTitle?: string;
    sections?: SectionConfig;
  },
): ReportData {
  if (!overrides) return data;
  return {
    ...data,
    branding: overrides.branding ? toReportBrandingPayload(overrides.branding) : data.branding,
    reportTitle: overrides.reportTitle ?? data.reportTitle,
    sections: overrides.sections ? toSectionConfigPayload(overrides.sections) : data.sections,
  };
}

export function getReportCoverageBadges(sections: SectionConfig, hasLinkedFolder: boolean) {
  const badges: { label: string; tone: "primary" | "muted" | "warning" }[] = [];
  badges.push({ label: "Web Scan", tone: "primary" });
  if (sections.recommendations) badges.push({ label: "Recommendations", tone: "muted" });
  if (sections.code_scan) {
    badges.push({
      label: hasLinkedFolder ? "Code Scan" : "Code Scan needs linked folder",
      tone: hasLinkedFolder ? "primary" : "warning",
    });
  }
  if (sections.analytics) badges.push({ label: "Analytics", tone: "muted" });
  if (sections.uptime) badges.push({ label: "Uptime", tone: "muted" });
  if (sections.deploys) badges.push({ label: "Deployments", tone: "muted" });
  return badges;
}

export function getHistoryCoverageBadges(
  sections: SectionConfig,
  summary: ReportHistorySummary | null,
) {
  const badges: { label: string; tone: "primary" | "muted" | "warning" }[] = [];
  badges.push({ label: "Web Scan", tone: "primary" });
  if (sections.recommendations) badges.push({ label: "Recommendations", tone: "muted" });
  if (sections.code_scan) {
    badges.push({
      label: summary?.has_code_scan ? "Code Scan included" : "Code Scan empty",
      tone: summary?.has_code_scan ? "primary" : "warning",
    });
  }
  if (sections.analytics) {
    badges.push({
      label: summary?.has_analytics ? "Analytics included" : "Analytics missing",
      tone: summary?.has_analytics ? "primary" : "warning",
    });
  }
  if (sections.uptime) {
    badges.push({
      label: summary?.has_uptime ? "Uptime included" : "Uptime missing",
      tone: summary?.has_uptime ? "primary" : "warning",
    });
  }
  if (sections.deploys) {
    badges.push({
      label: summary?.has_deploys ? "Deployments included" : "Deployments missing",
      tone: summary?.has_deploys ? "primary" : "warning",
    });
  }
  return badges;
}

export function parseHistorySections(sectionsJson: string | null): SectionConfig {
  if (!sectionsJson) return { ...DEFAULT_SECTIONS };
  const parsed = parseJsonRecord(sectionsJson);
  return parsed ? sectionsFromRecord(parsed) : { ...DEFAULT_SECTIONS };
}

export function buildReportHistorySummary(snapshot: ReportData): ReportHistorySummary {
  return {
    site_score: snapshot.siteScore.currentScore,
    site_critical: snapshot.siteScore.issuesCritical,
    site_high: snapshot.siteScore.issuesHigh,
    site_total: snapshot.siteScore.issuesTotal,
    latest_scan_date: snapshot.latestScanDate,
    has_code_scan: Boolean(snapshot.codeScan),
    code_critical: snapshot.codeScan?.criticalCount ?? null,
    code_high: snapshot.codeScan?.highCount ?? null,
    code_total: snapshot.codeScan?.issueCount ?? null,
    code_top_domain: snapshot.codeScan?.topDomain ?? null,
    code_domain_trend: snapshot.codeScan?.domainTrend ?? null,
    code_checked_at: snapshot.codeScan?.checkedAt ?? null,
    has_analytics: Boolean(snapshot.analytics),
    has_uptime: Boolean(snapshot.uptime),
    has_deploys: Boolean(snapshot.deploys),
  };
}

export function sectionsSignature(sections: SectionConfig): string {
  return JSON.stringify(sections);
}

export function parseHistorySummary(summaryJson: string | null): ReportHistorySummary | null {
  if (!summaryJson) return null;
  const parsed = parseJsonRecord(summaryJson);
  if (!parsed) return null;
  const siteScore = parsed.site_score ?? parsed.web_scan_score;
  const siteCritical = parsed.site_critical ?? parsed.web_scan_critical;
  const siteHigh = parsed.site_high ?? parsed.web_scan_high;
  const siteTotal = parsed.site_total ?? parsed.web_scan_total;
  if (
    typeof siteScore !== "number" ||
    typeof siteCritical !== "number" ||
    typeof siteHigh !== "number" ||
    typeof siteTotal !== "number"
  ) {
    return null;
  }
  return {
    site_score: siteScore,
    site_critical: siteCritical,
    site_high: siteHigh,
    site_total: siteTotal,
    latest_scan_date: nullableStringField(parsed.latest_scan_date, null),
    has_code_scan: parsed.has_code_scan === true || typeof parsed.code_score === "number",
    code_critical: typeof parsed.code_critical === "number" ? parsed.code_critical : null,
    code_high: typeof parsed.code_high === "number" ? parsed.code_high : null,
    code_total: typeof parsed.code_total === "number" ? parsed.code_total : null,
    code_top_domain: nullableStringField(parsed.code_top_domain, null),
    code_domain_trend: nullableStringField(parsed.code_domain_trend, null),
    code_checked_at: nullableStringField(parsed.code_checked_at, null),
    has_analytics: parsed.has_analytics === true,
    has_uptime: parsed.has_uptime === true,
    has_deploys: parsed.has_deploys === true,
  };
}

export function formatSavedReportDate(value: string | null): string | null {
  if (!value) return null;
  const parsed = new Date(value);
  if (!Number.isNaN(parsed.getTime())) {
    return parsed.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  }
  return value;
}

export function reportFormatLabel(outputFormat: string) {
  switch (outputFormat) {
    case "pdf":
      return "PDF";
    case "html":
      return "HTML";
    default:
      return "Preview";
  }
}
