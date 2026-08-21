import type { Tier } from "@/hooks/useTier";

const TIER_DISPLAY_NAME: Record<Tier, string> = {
  free: "Free",
  core: "Plus",
  pro: "Professional",
};

const TIER_BADGE_LABEL: Record<Tier, string> = {
  free: "FREE",
  core: "PLUS",
  pro: "PROFESSIONAL",
};

export function getTierDisplayName(tier: Tier): string {
  return TIER_DISPLAY_NAME[tier];
}

export function getTierBadgeLabel(tier: Tier): string {
  return TIER_BADGE_LABEL[tier];
}

export function normalizePlanDisplayName(planName: string | null | undefined): string {
  const trimmed = planName?.trim();
  if (!trimmed) return "";
  return trimmed.toLowerCase() === "pro" ? TIER_DISPLAY_NAME.pro : trimmed;
}
