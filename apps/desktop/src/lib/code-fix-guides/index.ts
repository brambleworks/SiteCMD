import type { FixEffort } from "../fix-guide-shared";
import { AI_FIX_GUIDES } from "./ai";
import { DATABASE_FIX_GUIDES } from "./database";
import { QUALITY_FIX_GUIDES } from "./quality";
import { SCAFFOLDING_FIX_GUIDES } from "./scaffolding";
import { SECURITY_FIX_GUIDES } from "./security";
import type { CodeFixGuideEntry } from "./types";

export interface CodeFixGuide {
  effort: FixEffort;
  effortMinutes: number;
  steps: string[];
}

// Bundled guides provide offline fallbacks when catalog content is unavailable.
const CODE_FIX_GUIDES: Record<string, CodeFixGuideEntry> = {
  ...AI_FIX_GUIDES,
  ...DATABASE_FIX_GUIDES,
  ...SECURITY_FIX_GUIDES,
  ...QUALITY_FIX_GUIDES,
  ...SCAFFOLDING_FIX_GUIDES,
};

/** Exact top-level guide keys, exported for registry parity tests. */
export const CODE_FIX_GUIDE_IDS: readonly string[] = Object.freeze(
  Object.keys(CODE_FIX_GUIDES).sort(),
);

export function getCodeFixGuide(producerRuleId: string): CodeFixGuide | null {
  const entry = CODE_FIX_GUIDES[producerRuleId];
  if (!entry) return null;
  return { effort: entry.effort, effortMinutes: entry.effortMinutes, steps: entry.default };
}
