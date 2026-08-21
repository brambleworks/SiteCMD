/** Shared microcopy for ignore, block, and snooze lifecycle actions. */

export interface TriageActionCopy {
  label: string;
  help: string;
}

export const ISSUE_TRIAGE_COPY = {
  ignore: {
    label: "Ignore",
    help: "Temporarily hide this finding until the next scan. If it is detected again, it returns to your active list and counts against your score.",
  },
  block: {
    label: "Block",
    help: "Permanently hide this finding across future scans. It stays out of your active list and score until you restore it from the Blocked issues view.",
  },
  snooze: {
    label: "Snooze",
    help: "Temporarily hide a finding. It returns to your active list later, so this is a reminder to revisit, not a decision that it does not apply.",
  },
} as const satisfies Record<string, TriageActionCopy>;

/** Score-impact note shown beside issue triage actions. */
export const TRIAGE_SCORE_RECOVERY_NOTE =
  "Your SiteCMD score counts only active issues. Ignore removes this finding until the next scan; Block keeps it out across future scans until you restore it.";
