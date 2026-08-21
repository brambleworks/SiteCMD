import { command } from "./invoke";
import type { PageRecord, ScanScopeWriteResult, SitemapResult } from "@/generated/ipc-bindings";
import type { ConnectedScopeSyncResult } from "@/generated/ipc-bindings-connected";

export function fetchSitemapManual(args: { sitemapUrl: string }): Promise<SitemapResult> {
  return command<SitemapResult>("fetch_sitemap_manual", args);
}

export function refreshSitemap(args: { siteId: number; url: string }): Promise<SitemapResult> {
  return command<SitemapResult>("refresh_sitemap", args);
}

export function saveSitePages(args: {
  siteId: number;
  urls: string[];
  source: string;
}): Promise<number> {
  return command<number>("save_site_pages", args);
}

export function getSitePages(args: { siteId: number }): Promise<PageRecord[]> {
  return command<PageRecord[]>("get_site_pages", args);
}

export function setSiteSitemapUrl(args: {
  siteId: number;
  sitemapUrl?: string | null;
}): Promise<void> {
  return command<void>("set_site_sitemap_url", args);
}

export function getOrCreateSiteId(args: { url: string; projectId?: number }): Promise<number> {
  return command<number>("get_or_create_site_id", args);
}

/** Returns canonical scan routes; an empty scope means entry page only. */
export function getScanScope(args: { siteId: number }): Promise<string[]> {
  return command<string[]>("get_scan_scope", args);
}

/** Replaces scan scope and returns its revision and canonical routes. */
export function setScanScope(args: {
  siteId: number;
  siteUrl: string;
  routes: string[];
}): Promise<ScanScopeWriteResult> {
  return command<ScanScopeWriteResult>("set_scan_scope", args);
}

/** Publish a committed local scope to the connected service. */
export function syncConnectedScanScope(args: {
  siteId: number;
}): Promise<ConnectedScopeSyncResult> {
  return command<ConnectedScopeSyncResult>("sync_connected_scan_scope", args);
}
