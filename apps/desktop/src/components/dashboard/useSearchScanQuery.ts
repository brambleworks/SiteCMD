import { useQuery } from "@tanstack/react-query";
import { getScanDetail, getScanHistory } from "@/lib/scan-execution-adapters";
import { queryKeys } from "@/lib/query/query-keys";
import { isActionableCheckResult, isPassingCheckResult } from "@/lib/issues";
import { coerceJsonRecord } from "@/lib/json-record";
import type { CategoryScore, CheckResult } from "@/lib/types";
import { buildSeoCategoryScore } from "./search-console-page-model";

export interface SearchScanSnapshot {
  score: CategoryScore | null;
  issues: CheckResult[];
  passedChecks: CheckResult[];
  /** Stack the scan detected, for stack-specific remediation variants. */
  detectedStack: Record<string, unknown> | null;
}

const EMPTY_SEARCH_SCAN: SearchScanSnapshot = {
  score: null,
  issues: [],
  passedChecks: [],
  detectedStack: null,
};

export function useSearchScanQuery(projectId: number, url: string) {
  const query = useQuery<SearchScanSnapshot>({
    queryKey: queryKeys.searchScan.forProject(projectId, url),
    queryFn: async () => {
      const history = await getScanHistory({ projectId, url, limit: 20 });
      const latestFullWebScan = history.find((entry) => entry.scanType === "health");
      if (!latestFullWebScan) return EMPTY_SEARCH_SCAN;
      const scan = await getScanDetail({ scanId: latestFullWebScan.id });
      if (!scan) throw new Error("scan detail unavailable");
      return {
        score:
          scan.categories.find((category) => category.category === "seo") ??
          buildSeoCategoryScore(scan.issues),
        issues: scan.issues.filter(
          (issue) => issue.category === "seo" && isActionableCheckResult(issue),
        ),
        passedChecks: scan.issues.filter(
          (issue) => issue.category === "seo" && isPassingCheckResult(issue),
        ),
        detectedStack: coerceJsonRecord(scan.detectedStack),
      };
    },
  });

  return {
    data: query.data,
    loading: query.isPending,
    refreshing: query.isFetching && !query.isPending,
    error: query.isError ? "Search checks could not load right now." : null,
    refetch: query.refetch,
  };
}
