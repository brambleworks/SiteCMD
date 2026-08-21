import { useQuery } from "@tanstack/react-query";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { getPageIssues } from "@/lib/issues";
import { queryKeys } from "@/lib/query/query-keys";

interface UseIssuesPageGroupsArgs {
  projectId: number;
  selectedPageUrl: string | null;
  url: string;
}

export function useIssuesPageGroups({ projectId, selectedPageUrl, url }: UseIssuesPageGroupsArgs) {
  const normalUrl = normalizeAppUrlForKey(url);
  const query = useQuery({
    queryKey: queryKeys.pageIssues.forPage(projectId, normalUrl, selectedPageUrl ?? ""),
    queryFn: () => getPageIssues(projectId, normalUrl, selectedPageUrl as string),
    enabled: selectedPageUrl != null,
  });

  return {
    pageGroups: selectedPageUrl ? (query.data ?? []) : [],
    loading: selectedPageUrl != null && query.isPending,
    error: query.isError ? "Page issue evidence could not load." : null,
    retry: () => void query.refetch(),
  };
}
