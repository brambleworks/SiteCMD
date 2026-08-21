import { useCallback, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { decideSiteBaseline, getOrCreateSiteId, getSiteBaseline } from "@/lib/commands";
import type { SiteBaseline, SiteBaselineField } from "@/generated/ipc-bindings";
import { queryKeys } from "@/lib/query/query-keys";

/** Read a site's baseline and apply revision-guarded decisions. */
export function useSiteBaseline(siteUrl: string | null, projectId?: number) {
  const queryClient = useQueryClient();
  const [refusal, setRefusal] = useState<string | null>(null);

  const siteQuery = useQuery({
    queryKey: queryKeys.settings.sitemapSite(siteUrl ?? "", projectId),
    queryFn: () => getOrCreateSiteId({ url: siteUrl as string, projectId }),
    enabled: Boolean(siteUrl),
  });
  const siteId = siteQuery.data ?? null;
  const baselineKey =
    siteId === null
      ? queryKeys.siteBaseline.all
      : queryKeys.siteBaseline.forScope(siteId, projectId, siteUrl ?? undefined);

  const query = useQuery({
    queryKey: baselineKey,
    queryFn: () =>
      getSiteBaseline({
        siteId: siteId as number,
        projectId,
        environmentScopeKey: siteUrl ?? undefined,
      }),
    enabled: siteId !== null,
  });

  const mutation = useMutation({
    mutationFn: (args: { field: SiteBaselineField; accept: boolean; revision: number }) =>
      decideSiteBaseline({
        siteId: siteId as number,
        field: args.field.field,
        basedOnRevision: args.revision,
        expectedDigest: args.field.changeDigest,
        accept: args.accept,
        projectId,
        environmentScopeKey: siteUrl ?? undefined,
      }),
    onSuccess: (result) => {
      setRefusal(result.applied ? null : result.message);
      if (siteId !== null) {
        void queryClient.invalidateQueries({ queryKey: baselineKey });
      }
    },
  });

  const decide = useCallback(
    (field: SiteBaselineField, accept: boolean) => {
      const baseline: SiteBaseline | undefined = query.data;
      if (!baseline || siteId === null) return;
      setRefusal(null);
      mutation.mutate({ field, accept, revision: baseline.revision });
    },
    [mutation, query.data, siteId],
  );

  return {
    baseline: query.data ?? null,
    loading: siteQuery.isLoading || query.isLoading,
    deciding: mutation.isPending,
    refusal,
    decide,
  };
}
