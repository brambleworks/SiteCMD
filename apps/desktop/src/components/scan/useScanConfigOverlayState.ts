import { useCallback, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useToast } from "@/hooks/useToast";

import {
  getOrCreateSiteId,
  getScanScope,
  getSitePages,
  refreshSitemap,
  setScanScope,
  syncConnectedScanScope,
} from "@/lib/commands";
import { errorMessage } from "@/lib/error-message";
import { getProjectCapabilities } from "@/lib/project-capabilities";
import { userFacingError } from "@/lib/user-facing-error";
import { queryKeys } from "@/lib/query/query-keys";

import {
  getDefaultPageUrl,
  pagesWithScopeRoutes,
  routeOf,
  scopeSelection,
  type PageRecord,
  type ScanConfig,
  type ScanMode,
} from "./scan-config-overlay-model";

interface UseScanConfigOverlayStateOptions {
  canUseAccessibilityDeepScan: boolean;
  initialAxeEnabled: boolean;
  initialScanType: ScanMode;
  onCancel: () => void;
  onStart: (config: ScanConfig) => void;
  projectPath?: string | null;
  projectId?: number;
  siteId?: number;
  siteUrl: string;
}

export function useScanConfigOverlayState({
  canUseAccessibilityDeepScan,
  initialAxeEnabled,
  initialScanType,
  onCancel,
  onStart,
  projectPath,
  projectId,
  siteId,
  siteUrl,
}: UseScanConfigOverlayStateOptions) {
  const [pages, setPages] = useState<PageRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [discovering, setDiscovering] = useState(false);
  const [inspectLocalDatabases, setInspectLocalDatabases] = useState(false);
  // Preserve the engine's scope refusal wording across every client.
  const [scopeError, setScopeError] = useState<string | null>(null);
  const [resolvedSiteId, setResolvedSiteId] = useState<number | undefined>(siteId);
  const queryClient = useQueryClient();
  const toast = useToast();

  const { hasSite, hasCode } = getProjectCapabilities({
    environmentUrl: siteUrl,
    projectFolder: projectPath,
  });
  // Code Scan requires only a linked folder.
  const canUseCodeScan = hasCode;

  // Fall back to the scan engine supported by the project's URL and folder.
  const requestedScanType: ScanMode =
    initialScanType === "code" && !canUseCodeScan ? "web" : initialScanType;
  const scanType: ScanMode =
    requestedScanType !== "code" && !hasSite && canUseCodeScan ? "code" : requestedScanType;
  const axeEnabled = scanType === "code" ? false : initialAxeEnabled && canUseAccessibilityDeepScan;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onCancel]);

  const resolveThenLoad = useCallback(
    async (options?: { force?: boolean }) => {
      // Code-only projects have no site scope to resolve.
      if (!hasSite) {
        setPages([]);
        setSelected(new Set());
        setLoading(false);
        return;
      }
      setLoading(true);
      try {
        const id: number =
          siteId ||
          (await queryClient.fetchQuery<number>({
            queryKey: queryKeys.settings.sitemapSite(siteUrl, projectId),
            queryFn: () => getOrCreateSiteId({ url: siteUrl, projectId }),
          }));
        setResolvedSiteId(id);
        // Discovery updates the shared sitemap cache used by Site Setup.
        const p = await queryClient.fetchQuery<PageRecord[]>({
          queryKey: queryKeys.settings.sitemapPages(id),
          queryFn: () => getSitePages({ siteId: id }),
          ...(options?.force ? { staleTime: 0 } : {}),
        });
        // Open on the stored scope used by unattended scans.
        const storedRoutes = (await getScanScope({ siteId: id }).catch(() => [] as string[])) ?? [];
        // A sitemap refresh can drop a page that is still in scope. It is
        // still scanned, so it still shows up in the list.
        setPages(pagesWithScopeRoutes(p, storedRoutes, siteUrl));
        const defaults =
          storedRoutes.length > 0
            ? scopeSelection(storedRoutes, siteUrl, p)
            : p.length > 0
              ? [getDefaultPageUrl(p, siteUrl)]
              : [siteUrl];
        setSelected(new Set(defaults));
      } catch {
        // Page discovery is optional until the first scan.
      } finally {
        setLoading(false);
      }
    },
    [hasSite, projectId, queryClient, siteId, siteUrl],
  );

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- resolves and loads the scan config asynchronously on open
    void resolveThenLoad();
  }, [resolveThenLoad]);

  const handleDiscover = useCallback(async () => {
    if (!resolvedSiteId) return;
    setDiscovering(true);
    try {
      await refreshSitemap({ siteId: resolvedSiteId, url: siteUrl });
      // Sitemap refresh invalidates the shared site-pages cache.
      await resolveThenLoad({ force: true });
    } catch {
      // Sitemap discovery failed - user can still enter URLs manually
    } finally {
      setDiscovering(false);
    }
  }, [resolvedSiteId, resolveThenLoad, siteUrl]);

  const filtered = useMemo(() => {
    if (!search) return pages;
    const q = search.toLowerCase();
    return pages.filter(
      (p) =>
        p.path.toLowerCase().includes(q) ||
        p.url.toLowerCase().includes(q) ||
        (p.title && p.title.toLowerCase().includes(q)),
    );
  }, [pages, search]);

  const selectAll = useCallback(() => setSelected(new Set(filtered.map((p) => p.url))), [filtered]);
  const selectNone = useCallback(() => setSelected(new Set()), []);

  const togglePage = useCallback((url: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(url)) next.delete(url);
      else next.add(url);
      return next;
    });
  }, []);

  const handleStart = useCallback(async () => {
    if (scanType === "code") {
      onStart({
        urls: hasSite ? [siteUrl] : [],
        axeEnabled: false,
        inspectLocalDatabases,
        scanType,
      });
      return;
    }
    let urls = selected.size > 0 ? Array.from(selected) : [siteUrl];
    let connectedScopeSync: Promise<unknown> | null = null;
    // Persist the scope before scanning so scheduled and immediate runs agree.
    if (resolvedSiteId) {
      try {
        const storedScope = await setScanScope({
          siteId: resolvedSiteId,
          siteUrl,
          routes: urls.map((url) => routeOf(url, siteUrl)),
        });
        urls = scopeSelection(storedScope.routes, siteUrl, pages);
        connectedScopeSync = syncConnectedScanScope({ siteId: resolvedSiteId });
        if (projectId !== undefined) {
          void queryClient.invalidateQueries({
            queryKey: queryKeys.settings.connectedStatus(projectId, siteUrl),
          });
        }
        setScopeError(null);
      } catch (error) {
        setScopeError(userFacingError(error, "Your change was not saved. Try again."));
        return;
      }
    }
    onStart({
      urls,
      axeEnabled: canUseAccessibilityDeepScan ? axeEnabled : false,
      inspectLocalDatabases,
      scanType,
    });
    if (connectedScopeSync) {
      void connectedScopeSync
        .then(async () => {
          if (projectId === undefined) return;
          await Promise.all([
            queryClient.invalidateQueries({
              queryKey: queryKeys.settings.connectedStatus(projectId, siteUrl),
            }),
            queryClient.invalidateQueries({
              queryKey: queryKeys.settings.connectedRemoteState(projectId, siteUrl),
            }),
          ]);
        })
        .catch((error) => {
          toast.warning("Local scope saved; connected scope still needs sync", errorMessage(error));
        });
    }
  }, [
    axeEnabled,
    canUseAccessibilityDeepScan,
    hasSite,
    inspectLocalDatabases,
    onStart,
    pages,
    projectId,
    queryClient,
    resolvedSiteId,
    scanType,
    selected,
    siteUrl,
    toast,
  ]);

  const fullScanCoverage = canUseCodeScan
    ? hasSite
      ? { label: "Full", description: "Web + Code" }
      : { label: "Full Code", description: "All code categories" }
    : { label: "Full Web", description: "All web categories" };

  return {
    axeEnabled,
    canUseCodeScan,
    discovering,
    filtered,
    fullScanDescription: fullScanCoverage.description,
    fullScanLabel: fullScanCoverage.label,
    hasSite,
    inspectLocalDatabases,
    handleDiscover,
    handleStart,
    hasPages: pages.length > 0,
    loading,
    pages,
    scanType,
    search,
    selectAll,
    selected,
    selectNone,
    setInspectLocalDatabases,
    scopeError,
    setSearch,
    togglePage,
  };
}
