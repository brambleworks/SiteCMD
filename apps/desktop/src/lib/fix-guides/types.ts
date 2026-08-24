import type { FixGuideMeta } from "../fix-guide-shared";

// Baseline guides are short and stack-neutral; richer variants arrive in signed packs.
export interface FixGuideEntry extends FixGuideMeta {
  /** One sentence, under 160 characters, that a non-engineer understands before the steps. */
  lead: string;
  default: string[];
}
