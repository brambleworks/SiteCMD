import type { FixEffort } from "../fix-guide-shared";

// Baseline guides are short and stack-neutral; richer variants arrive in signed packs.
export interface CodeFixGuideEntry {
  effort: FixEffort;
  effortMinutes: number;
  /** One sentence, under 160 characters, that a non-engineer understands before the steps. */
  lead: string;
  default: string[];
}
