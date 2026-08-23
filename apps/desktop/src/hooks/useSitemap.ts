import { useCallback, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  fetchSitemapManual,
  getOrCreateSiteId,
  getSitePages,
  refreshSitemap,
  saveSitePages,
  setSiteSitemapUrl,
} from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import { userFacingError } from "@/lib/user-facing-error";

interface PageRecord {
  id: number;
  siteId: number;
  url: string;
  path: string;
  title: string | null;
  lastSeenAt: string;
  source: string;
}

type SitemapState =
  "idle" | "discovering" | "found" | "not_found" | "manual_entry" | "no_sitemap" | "error";

export function useSitemap(
  siteUrl: string | undefined,
  siteId: number | undefined,
  projectId?: number,
) {
  const queryClient = useQueryClient();
  const [state, setState] = useState<SitemapState>("idle");
  const [sourceUrl, setSourceUrl] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const siteQuery = useQuery<number>({
    queryKey: queryKeys.settings.sitemapSite(siteUrl ?? "", projectId),
    queryFn: () => getOrCreateSiteId({ url: siteUrl as string, projectId }),
    enabled: siteId == null && Boolean(siteUrl),
  });
  const resolvedSiteId = siteId ?? siteQuery.data;
  const pagesQuery = useQuery<PageRecord[]>({
    queryKey: queryKeys.settings.sitemapPages(resolvedSiteId ?? 0),
    queryFn: async () => {
      const pages = await getSitePages({ siteId: resolvedSiteId as number });
      return Array.isArray(pages) ? (pages as PageRecord[]) : [];
    },
    enabled: resolvedSiteId != null,
  });
  const pages = pagesQuery.data ?? [];
  const queryFailed = siteQuery.isError || pagesQuery.isError;
  const effectiveState: SitemapState = queryFailed
    ? "error"
    : state === "idle" && pages.length > 0
      ? "found"
      : state;

  const reloadPages = useCallback(async () => {
    if (!resolvedSiteId) return [];
    const result = await queryClient.fetchQuery({
      queryKey: queryKeys.settings.sitemapPages(resolvedSiteId),
      queryFn: () => getSitePages({ siteId: resolvedSiteId }),
      staleTime: 0,
    });
    return Array.isArray(result) ? result : [];
  }, [queryClient, resolvedSiteId]);

  const discover = useCallback(async () => {
    if (!siteUrl || !resolvedSiteId) return;
    setState("discovering");
    setActionError(null);
    try {
      const result = await refreshSitemap({ siteId: resolvedSiteId, url: siteUrl });
      if (result.status === "found") {
        setSourceUrl(result.sourceUrl);
        await reloadPages();
        setState("found");
      } else {
        setState("not_found");
      }
    } catch (error) {
      setState("error");
      setActionError(userFacingError(error, "Try again in a moment."));
    }
  }, [reloadPages, resolvedSiteId, siteUrl]);

  const submitManualUrl = useCallback(
    async (manualUrl: string) => {
      if (!resolvedSiteId) return;
      setState("discovering");
      setActionError(null);
      try {
        const result = await fetchSitemapManual({ sitemapUrl: manualUrl });
        if (result.status === "found" && result.urls.length > 0) {
          await saveSitePages({ siteId: resolvedSiteId, urls: result.urls, source: "manual" });
          await setSiteSitemapUrl({ siteId: resolvedSiteId, sitemapUrl: manualUrl });
          setSourceUrl(manualUrl);
          await reloadPages();
          setState("found");
        } else {
          setState("not_found");
          setActionError("No pages found at that URL. Check that it's a valid sitemap.");
        }
      } catch (error) {
        setState("error");
        setActionError(userFacingError(error, "Try again in a moment."));
      }
    },
    [reloadPages, resolvedSiteId],
  );

  return {
    state: effectiveState,
    pages,
    sourceUrl,
    error: actionError ?? (queryFailed ? "Saved sitemap pages could not load." : null),
    loading:
      Boolean(siteUrl) &&
      ((siteId == null && siteQuery.isPending) || (resolvedSiteId != null && pagesQuery.isPending)),
    siteId: resolvedSiteId,
    discover,
    submitManualUrl,
    showManualEntry: () => setState("manual_entry"),
    showNoSitemap: () => setState("no_sitemap"),
    reset: () => setState("idle"),
    refresh: discover,
  };
}
