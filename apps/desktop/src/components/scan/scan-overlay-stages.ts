import type { LucideIcon } from "lucide-react";
import {
  CheckCircle,
  Eye,
  FileCode,
  Fingerprint,
  Gauge,
  Globe,
  Search,
  Shield,
  Sparkles,
} from "lucide-react";
import {
  CATEGORY_CSS_VAR,
  CATEGORY_LABELS,
  CATEGORY_SHORT_LABELS,
  CATEGORY_TEXT,
} from "@/lib/tokens";

export interface ScanStage {
  key: string;
  label: string;
  detail?: string;
  icon: LucideIcon;
  textClass?: string;
  color?: string;
  indicatorClass?: string;
  pingClass?: string;
}

const BRAND_STAGE_TONE = { indicatorClass: "bg-primary", pingClass: "scan-ring--brand" } as const;
const SECURITY_STAGE_TONE = {
  indicatorClass: "bg-cat-security",
  pingClass: "scan-ring--security",
} as const;
const SEO_STAGE_TONE = { indicatorClass: "bg-cat-seo", pingClass: "scan-ring--seo" } as const;
const PERFORMANCE_STAGE_TONE = {
  indicatorClass: "bg-cat-performance",
  pingClass: "scan-ring--performance",
} as const;
const ACCESSIBILITY_STAGE_TONE = {
  indicatorClass: "bg-cat-accessibility",
  pingClass: "scan-ring--accessibility",
} as const;
const COMPLIANCE_STAGE_TONE = {
  indicatorClass: "bg-cat-compliance",
  pingClass: "scan-ring--compliance",
} as const;
const POLISH_STAGE_TONE = {
  indicatorClass: "bg-cat-polish",
  pingClass: "scan-ring--polish",
} as const;

export const WEB_SCAN_STAGES: ScanStage[] = [
  {
    key: "fetch",
    label: "Fetch",
    detail: "Loading the page, response headers, and core document.",
    icon: Globe,
    textClass: "text-primary",
    color: "var(--brand)",
    ...BRAND_STAGE_TONE,
  },
  {
    key: "security",
    label: CATEGORY_LABELS.security,
    detail: "Checking HTTPS, headers, redirects, cookies, and exposed files.",
    icon: Shield,
    textClass: CATEGORY_TEXT.security,
    color: CATEGORY_CSS_VAR.security,
    ...SECURITY_STAGE_TONE,
  },
  {
    key: "seo",
    label: CATEGORY_LABELS.seo,
    detail: "Reviewing crawlability, metadata, canonical tags, and content signals.",
    icon: Search,
    textClass: CATEGORY_TEXT.seo,
    color: CATEGORY_CSS_VAR.seo,
    ...SEO_STAGE_TONE,
  },
  {
    key: "performance",
    label: CATEGORY_LABELS.performance,
    detail: "Checking compression, caching, assets, and loading signals.",
    icon: Gauge,
    textClass: CATEGORY_TEXT.performance,
    color: CATEGORY_CSS_VAR.performance,
    ...PERFORMANCE_STAGE_TONE,
  },
  {
    key: "accessibility",
    label: CATEGORY_LABELS.accessibility,
    detail: "Checking document structure, labels, keyboard support, and landmarks.",
    icon: Eye,
    textClass: CATEGORY_TEXT.accessibility,
    color: CATEGORY_CSS_VAR.accessibility,
    ...ACCESSIBILITY_STAGE_TONE,
  },
  {
    key: "compliance",
    // The strip fits seven stages side by side, so this one takes the short
    // name while its neighbours are already short enough.
    label: CATEGORY_SHORT_LABELS.compliance,
    detail: "Checking privacy notices, consent signals, trackers, and policy pages.",
    icon: Fingerprint,
    textClass: CATEGORY_TEXT.compliance,
    color: CATEGORY_CSS_VAR.compliance,
    ...COMPLIANCE_STAGE_TONE,
  },
  {
    key: "polish",
    label: CATEGORY_LABELS.polish,
    detail: "Looking for AI-written copy, default styling, layout rough edges, and UX tells.",
    icon: Sparkles,
    textClass: CATEGORY_TEXT.polish,
    color: CATEGORY_CSS_VAR.polish,
    ...POLISH_STAGE_TONE,
  },
  {
    key: "browser",
    label: "Browser",
    detail: "Running browser-rendered accessibility and runtime measurements.",
    icon: Gauge,
    textClass: CATEGORY_TEXT.performance,
    color: CATEGORY_CSS_VAR.performance,
    ...PERFORMANCE_STAGE_TONE,
  },
];

export const CODE_SCAN_STAGES: ScanStage[] = [
  {
    key: "collect-files",
    label: "Project Files",
    detail: "Finding source files and project config that belong in the audit.",
    icon: FileCode,
    textClass: "text-primary",
    color: "var(--brand)",
    ...BRAND_STAGE_TONE,
  },
  {
    key: "analyze-source",
    label: "Source Code",
    detail: "Checking routes, auth, database access, AI usage, and risky patterns.",
    icon: Search,
    textClass: "text-primary",
    color: "var(--brand)",
    ...BRAND_STAGE_TONE,
  },
  {
    key: "supply-chain",
    label: "Dependencies",
    detail: "Reviewing package inventory, lockfiles, scripts, and dependency hygiene.",
    icon: Shield,
    textClass: CATEGORY_TEXT.security,
    color: CATEGORY_CSS_VAR.security,
    ...SECURITY_STAGE_TONE,
  },
  {
    key: "operations",
    label: "Release Setup",
    detail: "Checking CI, hooks, rollback notes, deploy safety, and runtime readiness.",
    icon: Gauge,
    textClass: CATEGORY_TEXT.performance,
    color: CATEGORY_CSS_VAR.performance,
    ...PERFORMANCE_STAGE_TONE,
  },
  {
    key: "save",
    label: "Saving Results",
    detail: "Persisting issues and updating your unified issue list.",
    icon: CheckCircle,
    textClass: "text-primary",
    color: "var(--brand)",
    ...BRAND_STAGE_TONE,
  },
  {
    key: "summary",
    label: "Summary",
    detail: "Finalizing results and preparing your issue list.",
    icon: Sparkles,
    textClass: "text-primary",
    color: "var(--brand)",
    ...BRAND_STAGE_TONE,
  },
];
