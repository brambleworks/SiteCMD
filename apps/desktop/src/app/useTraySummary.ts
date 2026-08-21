import { useEffect } from "react";
import { updateTraySummary } from "@/lib/commands";
import { MS_PER_DAY } from "@/lib/format";
import { useRunningJobsCount } from "@/lib/jobs";
import type { ProjectRecord } from "@/hooks/useProject";
import type { DesktopPromptEntry } from "@/lib/desktop-prompts";

export function useTraySummary({
  desktopPrompts,
  projects,
}: {
  desktopPrompts: DesktopPromptEntry[];
  projects: ProjectRecord[];
}) {
  // Count-only jobs subscription: scan-progress ticks keep the count stable,
  // so they never re-render the shell component hosting this hook.
  const runningCount = useRunningJobsCount();
  useEffect(() => {
    const promptSites = new Set(desktopPrompts.map((entry) => entry.projectId)).size;
    const attentionSites = new Set<number>();

    for (const project of projects) {
      const primaryEnv = project.environments[0];
      if (!primaryEnv) continue;
      const hasAttention = primaryEnv.lastScannedAt
        ? Date.now() - new Date(primaryEnv.lastScannedAt).getTime() > 3 * MS_PER_DAY
        : false;
      if (hasAttention) attentionSites.add(project.id);
    }
    for (const entry of desktopPrompts) {
      attentionSites.add(entry.projectId);
    }

    void updateTraySummary({
      attentionCount: attentionSites.size,
      pendingCount: 0,
      promptCount: promptSites,
      runningCount,
    }).catch(() => {});
  }, [desktopPrompts, projects, runningCount]);
}
