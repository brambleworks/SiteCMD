import { getScanHistory, getSessionHistory } from "@/lib/scan-execution-adapters";
import type { ScanSessionSummary } from "@/hooks/useHistory";

export async function loadLatestWebScanId(
  projectId: number | null,
  url: string,
): Promise<number | null> {
  try {
    const latest = await getScanHistory({ projectId, url, limit: 1 });
    return latest[0]?.id ?? null;
  } catch {
    return null;
  }
}

export async function loadLatestSessionSummary(
  projectId: number | null,
  url: string,
): Promise<ScanSessionSummary | null> {
  try {
    const latest = await getSessionHistory({ projectId, url, limit: 1 });
    return latest[0] ?? null;
  } catch {
    return null;
  }
}
