import type { FixEffort } from "../fix-guide-shared";

// Baseline guides are short and stack-neutral; richer variants arrive in signed packs.
export interface CodeFixGuideEntry {
  effort: FixEffort;
  effortMinutes: number;
  default: string[];
}
