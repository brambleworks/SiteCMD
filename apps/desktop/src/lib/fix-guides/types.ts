import type { FixGuideMeta } from "../fix-guide-shared";

// Baseline guides are short and stack-neutral; richer variants arrive in signed packs.
export interface FixGuideEntry extends FixGuideMeta {
  default: string[];
}
