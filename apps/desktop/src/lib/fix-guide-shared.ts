export type FixEffort = "quick" | "moderate" | "involved";

export interface FixGuideMeta {
  effort: FixEffort;
  effortMinutes: number;
}

export function getEffortLabel(effort: FixEffort): string {
  switch (effort) {
    case "quick":
      return "~5 min fix";
    case "moderate":
      return "~15 min fix";
    case "involved":
      return "30+ min fix";
  }
}
