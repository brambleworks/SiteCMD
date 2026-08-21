import { getResolvedIssues as getResolvedIssuesCmd } from "@/lib/commands";

export interface ResolvedIssue {
  checkId: string;
  title: string;
  category: string;
  severity: string;
  resolvedScanId: number | null;
  resolvedAt: string;
  firstSeenScanId: number | null;
  firstSeenAt: string;
  durationHours: number | null;
  recurrenceCount: number;
}

export async function getResolvedIssues(
  projectId: number,
  url: string,
  limit: number,
): Promise<ResolvedIssue[]> {
  return getResolvedIssuesCmd({ projectId, url, limit });
}
