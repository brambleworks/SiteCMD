import { getCurrentScore } from "@/lib/commands";
import type { ScoreSnapshot } from "@/lib/types";

export async function loadCurrentScoreSnapshot(
  projectId: number,
  envUrl: string | null,
): Promise<ScoreSnapshot> {
  return getCurrentScore({ projectId, envUrl });
}

export function currentScoreIssueCount(score: ScoreSnapshot): number {
  return score.criticalCount + score.highCount + score.mediumCount + score.lowCount;
}
