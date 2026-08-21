import { useQuery } from "@tanstack/react-query";
import { getCodeScanDetail } from "@/lib/scan-execution-adapters";
import { queryKeys } from "@/lib/query/query-keys";
import type { CodeScanResult, CodeScanSummary } from "@/lib/types";

/** Resolves embedded or shared-query Code Scan detail for the dashboard. */
export function useDashboardCodeScanDetail({
  latestCodeScanDetail,
  latestCodeScanSummary,
}: {
  latestCodeScanDetail: CodeScanResult | null;
  latestCodeScanSummary: CodeScanSummary | null;
}): CodeScanResult | null {
  const scanId = latestCodeScanSummary?.id ?? null;
  const enabled = !latestCodeScanDetail && scanId != null;

  const { data } = useQuery({
    // `scanId ?? -1` is never fetched while disabled; the guard keeps the key
    // well-typed without a fetch.
    queryKey: queryKeys.scanExecution.detail(scanId ?? -1),
    queryFn: () => getCodeScanDetail({ scanId: scanId! }).then((result) => result ?? null),
    enabled,
  });

  if (latestCodeScanDetail) return latestCodeScanDetail;
  return data ?? null;
}
