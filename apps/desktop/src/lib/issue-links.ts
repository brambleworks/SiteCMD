import { createIssueLink } from "@/lib/commands";
import type { CheckResult, IssueLink } from "@/lib/types";

export type TrackerProvider = "github" | "jira";

export const TRACKER_LABELS: Record<TrackerProvider, string> = {
  github: "GitHub",
  jira: "Jira",
};

/** Tracker integrations that are configured and enabled, in a stable order. */
interface TrackerIntegrationConfig {
  integrationType: string;
  enabled: boolean;
}

export function enabledTrackerProviders(
  configs: readonly TrackerIntegrationConfig[],
): TrackerProvider[] {
  const providers: TrackerProvider[] = [];
  for (const provider of ["github", "jira"] as const) {
    if (configs.some((c) => c.integrationType === provider && c.enabled)) {
      providers.push(provider);
    }
  }
  return providers;
}

interface SendIssueToTrackerArgs {
  projectId: number;
  scanId: number;
  provider: TrackerProvider;
  issue: CheckResult;
  /** Estimated score points this issue costs; rendered in the ticket body. */
  estimatedImpact: number;
}

/** Create a ticket only after Rust verifies and loads its local issue context. */
export async function sendIssueToTracker(args: SendIssueToTrackerArgs): Promise<IssueLink> {
  const { projectId, scanId, provider, issue, estimatedImpact } = args;

  return createIssueLink({
    projectId,
    checkId: issue.checkId,
    scanId,
    provider,
    estimatedImpact: Math.max(0, Math.round(estimatedImpact)),
  });
}
