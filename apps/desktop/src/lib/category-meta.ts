import { Shield, Zap, Search, Eye, Scale, Sparkles, Settings, type LucideIcon } from "lucide-react";
import type { ScanCategory } from "@/lib/types";

interface ScanCategoryMeta {
  label: string;
  shortLabel: string;
  description: string;
  icon: LucideIcon;
  /** CSS variable name (bare, no `var` wrapper) for the category accent. */
  accentVar: string;
}

export const CATEGORY_META: Record<ScanCategory, ScanCategoryMeta> = {
  security: {
    label: "Security",
    shortLabel: "Security",
    description: "Headers, exposed files, forms, auth flows, and browser-facing security checks.",
    icon: Shield,
    accentVar: "--cat-security",
  },
  performance: {
    label: "Performance",
    shortLabel: "Perf",
    description: "Speed, asset weight, loading order, caching, and Core Web Vitals pressure.",
    icon: Zap,
    accentVar: "--cat-performance",
  },
  seo: {
    label: "SEO",
    shortLabel: "SEO",
    description: "Metadata, crawlability, internal signals, and search-result readiness.",
    icon: Search,
    accentVar: "--cat-seo",
  },
  accessibility: {
    label: "Accessibility",
    shortLabel: "Accessibility",
    description: "Keyboard access, semantics, labels, contrast, and assistive-tech compatibility.",
    icon: Eye,
    accentVar: "--cat-accessibility",
  },
  compliance: {
    label: "Legal",
    shortLabel: "Legal",
    description: "Privacy, consent, policy coverage, and launch-time legal/compliance signals.",
    icon: Scale,
    accentVar: "--cat-compliance",
  },
  polish: {
    label: "Polish",
    shortLabel: "Polish",
    description:
      "Broken trust cues, sloppy UX details, and obvious quality signals users notice fast.",
    icon: Sparkles,
    accentVar: "--cat-polish",
  },
  config: {
    label: "Config",
    shortLabel: "Config",
    description: "Robots, sitemap, redirects, environment setup, and technical site configuration.",
    icon: Settings,
    accentVar: "--cat-config",
  },
};
