import {
  clearSessionSnapshotCache,
  parseSnapshotCacheEntry,
  readFreshSessionSnapshot,
  snapshotCacheKey,
  writeSessionSnapshot,
  SNAPSHOT_CACHE_TTL_MS,
  type SnapshotCacheEntry,
} from "@/lib/project-summary-cache";
import {
  invalidateLatestCodeScanSnapshot,
  mergePrimedCodeScanForAccess,
  primeLatestCodeScanSnapshot,
  shouldPreferPrimed,
} from "@/lib/project-summary-code-scan";
import type {
  DashboardReferenceSignals,
  DashboardSnapshot,
  ProjectNavBadgeSnapshot,
  ProjectSignalSnapshot,
  TodayProjectWorkSummary,
} from "@/lib/project-summary-types";
import { queryKeys } from "@/lib/query/query-keys";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import type { QueryClient } from "@tanstack/react-query";
import {
  getAllProjectsWorkSummary as getAllProjectsWorkSummaryCmd,
  getDashboardReferenceSignals as getDashboardReferenceSignalsCmd,
  getDashboardSnapshot as getDashboardSnapshotCmd,
  getProjectNavBadgeSnapshot as getProjectNavBadgeSnapshotCmd,
  getProjectSignalSnapshot as getProjectSignalSnapshotCmd,
  invalidateProjectSignalSnapshot as invalidateProjectSignalSnapshotCmd,
} from "@/lib/commands";
import type { UpdateReport } from "@/lib/types";

export type {
  DashboardCodeTrendPoint,
  DashboardPagespeedReport,
  DashboardReferenceSignals,
  DashboardSnapshot,
  DashboardWorkflowRun,
  LatestCodeScanSnapshot,
  ProjectNavBadgeSnapshot,
  ProjectSignalSnapshot,
  ProjectWorkItem,
  ProjectWorkQueue,
  ProjectWorkSummary,
  SearchRegressionSignal,
  TodayProjectWorkSummary,
  WorkItemStatus,
} from "@/lib/project-summary-types";
export { invalidateLatestCodeScanSnapshot, primeLatestCodeScanSnapshot, shouldPreferPrimed };

interface SnapshotReadOptions {
  forceRefresh?: boolean;
  bypassCache?: boolean;
  includeCodeScanDetail?: boolean;
}

interface DashboardReferenceSignalOptions {
  includePsi?: boolean;
  bypassCache?: boolean;
}

// QueryClient is the session cache; sessionStorage survives webview reloads.
interface ReferenceSignalsCacheEntry {
  snapshot: DashboardReferenceSignals;
  cachedAt: number;
}
const DASHBOARD_SNAPSHOT_STORAGE_PREFIX = "sitecmd:dashboard-snapshot:";
const DASHBOARD_REFERENCE_SIGNALS_STORAGE_PREFIX = "sitecmd:dashboard-reference-signals:";
const NAV_BADGE_SNAPSHOT_STORAGE_PREFIX = "sitecmd:nav-badge-snapshot:";
const DASHBOARD_REFERENCE_SIGNALS_CACHE_TTL_MS = 30 * 60 * 1000;

export function clearProjectSignalSessionCache(projectId: number) {
  if (typeof window === "undefined") return;
  const projectPrefixes = [
    DASHBOARD_SNAPSHOT_STORAGE_PREFIX,
    DASHBOARD_REFERENCE_SIGNALS_STORAGE_PREFIX,
    NAV_BADGE_SNAPSHOT_STORAGE_PREFIX,
  ].map((prefix) => `${prefix}${projectId}:`);
  try {
    for (let index = window.sessionStorage.length - 1; index >= 0; index -= 1) {
      const key = window.sessionStorage.key(index);
      if (key && projectPrefixes.some((prefix) => key.startsWith(prefix))) {
        window.sessionStorage.removeItem(key);
      }
    }
  } catch {
    // best effort only
  }
}

// Keep heavy scan details out of sessionStorage; partial snapshots refetch them.
function slimProjectSignalsForSession(signals: ProjectSignalSnapshot): ProjectSignalSnapshot {
  if (signals.codeScanDetail == null) return signals;
  return { ...signals, codeScanDetail: null };
}

function slimDashboardSnapshotForSession(snapshot: DashboardSnapshot): DashboardSnapshot {
  return {
    ...snapshot,
    latestDetail: null,
    previousDetail: null,
    signals: slimProjectSignalsForSession(snapshot.signals),
  };
}

function slimNavBadgeSnapshotForSession(
  snapshot: ProjectNavBadgeSnapshot,
): ProjectNavBadgeSnapshot {
  return {
    ...snapshot,
    signals: slimProjectSignalsForSession(snapshot.signals),
  };
}

// QueryClient owns full snapshots; session storage keeps slim reload fallbacks.
function readFreshClientSnapshot<T>(
  queryClient: QueryClient,
  queryKey: readonly unknown[],
  storagePrefix: string,
  sessionKey: string,
): SnapshotCacheEntry<T> | null {
  // Imperative reads must honor invalidation before accepting cached data.
  if (queryClient.getQueryState(queryKey)?.isInvalidated) {
    queryClient.removeQueries({ queryKey, exact: true });
    clearSessionSnapshotCache(storagePrefix, sessionKey);
    return null;
  }
  const live = queryClient.getQueryData<SnapshotCacheEntry<T>>(queryKey);
  if (live) {
    if (Date.now() - live.cachedAt <= SNAPSHOT_CACHE_TTL_MS) return live;
    queryClient.removeQueries({ queryKey, exact: true });
    clearSessionSnapshotCache(storagePrefix, sessionKey);
    return null;
  }
  const persisted = readFreshSessionSnapshot<T>(storagePrefix, sessionKey);
  if (persisted) {
    // Keep a restored entry warm for the next in-session read.
    queryClient.setQueryData<SnapshotCacheEntry<T>>(queryKey, persisted);
  }
  return persisted;
}

function writeClientSnapshot<T>(
  queryClient: QueryClient,
  queryKey: readonly unknown[],
  storagePrefix: string,
  sessionKey: string,
  snapshot: T,
  slimForSession?: (snapshot: T) => T,
  partial?: boolean,
): T {
  const cachedAt = Date.now();
  const entry: SnapshotCacheEntry<T> = partial
    ? { snapshot, cachedAt, partial: true }
    : { snapshot, cachedAt };
  queryClient.setQueryData<SnapshotCacheEntry<T>>(queryKey, entry);
  writeSessionSnapshot(storagePrefix, sessionKey, snapshot, cachedAt, slimForSession);
  return snapshot;
}

// Keep the storage key stable so existing entries still hydrate.
function referenceSignalsCacheKey(
  projectId: number,
  url: string | null | undefined,
  includePsi: boolean,
) {
  return `${snapshotCacheKey(projectId, url)}:${includePsi ? "psi" : "base"}`;
}

// Normalize URLs so trailing-slash variants share one entry.
function referenceSignalsQueryKey(
  projectId: number,
  url: string | null | undefined,
  includePsi: boolean,
) {
  return queryKeys.projectSummary.referenceSignals(
    projectId,
    normalizeAppUrlForKey(url),
    includePsi,
  );
}

function readSessionReferenceSignalsCache(key: string) {
  if (typeof window === "undefined") return null;
  const storageKey = `${DASHBOARD_REFERENCE_SIGNALS_STORAGE_PREFIX}${key}`;
  try {
    const raw = window.sessionStorage.getItem(storageKey);
    if (!raw) return null;
    const parsed = parseSnapshotCacheEntry<DashboardReferenceSignals>(JSON.parse(raw) as unknown);
    if (!parsed) {
      window.sessionStorage.removeItem(storageKey);
    }
    return parsed;
  } catch {
    window.sessionStorage.removeItem(storageKey);
    return null;
  }
}

function writeSessionReferenceSignalsCache(key: string, snapshot: DashboardReferenceSignals) {
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage.setItem(
      `${DASHBOARD_REFERENCE_SIGNALS_STORAGE_PREFIX}${key}`,
      JSON.stringify({
        snapshot,
        cachedAt: Date.now(),
      }),
    );
  } catch {
    // best effort only
  }
}

function clearReferenceSignalsCacheKey(
  queryClient: QueryClient,
  projectId: number,
  url: string | null | undefined,
  includePsi: boolean,
) {
  queryClient.removeQueries({
    queryKey: referenceSignalsQueryKey(projectId, url, includePsi),
    exact: true,
  });
  clearSessionSnapshotCache(
    DASHBOARD_REFERENCE_SIGNALS_STORAGE_PREFIX,
    referenceSignalsCacheKey(projectId, url, includePsi),
  );
}

function readFreshReferenceSignalsCache(
  queryClient: QueryClient,
  projectId: number,
  url: string | null | undefined,
  includePsi: boolean,
): DashboardReferenceSignals | null {
  const queryKey = referenceSignalsQueryKey(projectId, url, includePsi);
  if (queryClient.getQueryState(queryKey)?.isInvalidated) {
    clearReferenceSignalsCacheKey(queryClient, projectId, url, includePsi);
    return null;
  }
  const cached =
    queryClient.getQueryData<ReferenceSignalsCacheEntry>(queryKey) ??
    readSessionReferenceSignalsCache(referenceSignalsCacheKey(projectId, url, includePsi));
  if (!cached) return null;
  if (Date.now() - cached.cachedAt > DASHBOARD_REFERENCE_SIGNALS_CACHE_TTL_MS) {
    clearReferenceSignalsCacheKey(queryClient, projectId, url, includePsi);
    return null;
  }
  // Keep a valid entry warm for the next in-session read.
  queryClient.setQueryData<ReferenceSignalsCacheEntry>(queryKey, cached);
  return cached.snapshot;
}

function writeReferenceSignalsCache(
  queryClient: QueryClient,
  projectId: number,
  url: string | null | undefined,
  includePsi: boolean,
  snapshot: DashboardReferenceSignals,
): DashboardReferenceSignals {
  const entry: ReferenceSignalsCacheEntry = {
    snapshot,
    cachedAt: Date.now(),
  };
  queryClient.setQueryData<ReferenceSignalsCacheEntry>(
    referenceSignalsQueryKey(projectId, url, includePsi),
    entry,
  );
  writeSessionReferenceSignalsCache(referenceSignalsCacheKey(projectId, url, includePsi), snapshot);
  return snapshot;
}

function referenceSignalsAreCacheable(snapshot: DashboardReferenceSignals) {
  return snapshot.integrations.every((integration) => integration.error == null);
}

export async function getProjectSignalSnapshot(
  projectId: number,
  url?: string | null,
  options?: SnapshotReadOptions,
): Promise<ProjectSignalSnapshot> {
  // Assert the generated wire payload into the richer dashboard model at this boundary.
  const snapshot = (await getProjectSignalSnapshotCmd({
    projectId,
    url: url ?? null,
    forceRefresh: options?.forceRefresh ?? false,
    includeCodeScanDetail: options?.includeCodeScanDetail ?? true,
  })) as unknown as ProjectSignalSnapshot;
  return mergePrimedCodeScanForAccess(snapshot, options?.includeCodeScanDetail ?? true);
}

export async function getDashboardSnapshot(
  queryClient: QueryClient,
  projectId: number,
  url: string,
  options?: SnapshotReadOptions,
): Promise<DashboardSnapshot> {
  const queryKey = queryKeys.projectSummary.snapshot(projectId, normalizeAppUrlForKey(url));
  const sessionKey = snapshotCacheKey(projectId, url);
  if (!options?.forceRefresh && !options?.bypassCache) {
    const cached = readFreshClientSnapshot<DashboardSnapshot>(
      queryClient,
      queryKey,
      DASHBOARD_SNAPSHOT_STORAGE_PREFIX,
      sessionKey,
    );
    // Partial session entries support instant paint but require a detail refetch.
    if (cached && !cached.partial) {
      return {
        ...cached.snapshot,
        signals: mergePrimedCodeScanForAccess(
          cached.snapshot.signals,
          options?.includeCodeScanDetail ?? true,
        ),
      };
    }
  }
  const snapshot = writeClientSnapshot<DashboardSnapshot>(
    queryClient,
    queryKey,
    DASHBOARD_SNAPSHOT_STORAGE_PREFIX,
    sessionKey,
    (await getDashboardSnapshotCmd({
      projectId,
      url,
      forceRefresh: options?.forceRefresh ?? false,
    })) as unknown as DashboardSnapshot,
    slimDashboardSnapshotForSession,
  );
  return {
    ...snapshot,
    signals: mergePrimedCodeScanForAccess(snapshot.signals, options?.includeCodeScanDetail ?? true),
  };
}

export function peekDashboardSnapshot(
  queryClient: QueryClient,
  projectId: number,
  url: string,
): DashboardSnapshot | null {
  // Instant paint accepts partial session data while the full query refetches.
  const cached = readFreshClientSnapshot<DashboardSnapshot>(
    queryClient,
    queryKeys.projectSummary.snapshot(projectId, normalizeAppUrlForKey(url)),
    DASHBOARD_SNAPSHOT_STORAGE_PREFIX,
    snapshotCacheKey(projectId, url),
  );
  if (!cached) return null;
  return {
    ...cached.snapshot,
    signals: mergePrimedCodeScanForAccess(cached.snapshot.signals, true),
  };
}

export function peekDashboardReferenceSignals(
  queryClient: QueryClient,
  projectId: number,
  url: string,
  options?: DashboardReferenceSignalOptions,
): DashboardReferenceSignals | null {
  const includePsi = options?.includePsi ?? false;
  if (includePsi) {
    return readFreshReferenceSignalsCache(queryClient, projectId, url, true);
  }
  return (
    readFreshReferenceSignalsCache(queryClient, projectId, url, false) ??
    readFreshReferenceSignalsCache(queryClient, projectId, url, true)
  );
}

function withPrimedUpdates<T extends { signals: ProjectSignalSnapshot }>(
  snapshot: T,
  updates: UpdateReport,
  refreshedAt: string,
): T {
  return {
    ...snapshot,
    signals: {
      ...snapshot.signals,
      updates,
      updatesRefreshedAt: refreshedAt,
    },
  };
}

export function primeProjectUpdatesSnapshot(
  queryClient: QueryClient,
  projectId: number,
  url: string | null | undefined,
  updates: UpdateReport,
) {
  const cacheKey = snapshotCacheKey(projectId, url);
  const refreshedAt = new Date().toISOString();
  const dashboardQueryKey = queryKeys.projectSummary.snapshot(
    projectId,
    normalizeAppUrlForKey(url),
  );
  const dashboardEntry = readFreshClientSnapshot<DashboardSnapshot>(
    queryClient,
    dashboardQueryKey,
    DASHBOARD_SNAPSHOT_STORAGE_PREFIX,
    cacheKey,
  );
  if (dashboardEntry) {
    writeClientSnapshot<DashboardSnapshot>(
      queryClient,
      dashboardQueryKey,
      DASHBOARD_SNAPSHOT_STORAGE_PREFIX,
      cacheKey,
      withPrimedUpdates(dashboardEntry.snapshot, updates, refreshedAt),
      slimDashboardSnapshotForSession,
      // Priming updates into a session-restored entry must not launder away
      // its partial flag - the stripped detail fields still need a refetch.
      dashboardEntry.partial,
    );
  }

  const navBadgeQueryKey = queryKeys.projectSummary.navBadge(projectId, normalizeAppUrlForKey(url));
  const navBadgeEntry = readFreshClientSnapshot<ProjectNavBadgeSnapshot>(
    queryClient,
    navBadgeQueryKey,
    NAV_BADGE_SNAPSHOT_STORAGE_PREFIX,
    cacheKey,
  );
  if (navBadgeEntry) {
    writeClientSnapshot<ProjectNavBadgeSnapshot>(
      queryClient,
      navBadgeQueryKey,
      NAV_BADGE_SNAPSHOT_STORAGE_PREFIX,
      cacheKey,
      withPrimedUpdates(navBadgeEntry.snapshot, updates, refreshedAt),
      slimNavBadgeSnapshotForSession,
      navBadgeEntry.partial,
    );
  }
}

export async function getDashboardReferenceSignals(
  queryClient: QueryClient,
  projectId: number,
  url: string,
  options?: DashboardReferenceSignalOptions,
): Promise<DashboardReferenceSignals> {
  const includePsi = options?.includePsi ?? false;
  if (!options?.bypassCache) {
    const cached = peekDashboardReferenceSignals(queryClient, projectId, url, { includePsi });
    if (cached) return cached;
  }

  const response = (await getDashboardReferenceSignalsCmd({
    projectId,
    url,
    includePsi,
  })) as unknown as DashboardReferenceSignals;
  const cacheable = referenceSignalsAreCacheable(response);
  // PSI responses populate both PSI and base cache keys.
  for (const variant of includePsi ? [true, false] : [false]) {
    if (cacheable) {
      writeReferenceSignalsCache(queryClient, projectId, url, variant, response);
    } else {
      clearReferenceSignalsCacheKey(queryClient, projectId, url, variant);
    }
  }
  return response;
}

export async function getProjectNavBadgeSnapshot(
  queryClient: QueryClient,
  projectId: number,
  url: string,
  options?: Pick<SnapshotReadOptions, "forceRefresh">,
): Promise<ProjectNavBadgeSnapshot> {
  const queryKey = queryKeys.projectSummary.navBadge(projectId, normalizeAppUrlForKey(url));
  const sessionKey = snapshotCacheKey(projectId, url);
  if (!options?.forceRefresh) {
    const cached = readFreshClientSnapshot<ProjectNavBadgeSnapshot>(
      queryClient,
      queryKey,
      NAV_BADGE_SNAPSHOT_STORAGE_PREFIX,
      sessionKey,
    );
    // Nav badges do not use code detail, so partial session data is lossless.
    if (cached) {
      return {
        ...cached.snapshot,
        signals: mergePrimedCodeScanForAccess(cached.snapshot.signals, false),
      };
    }
  }
  const snapshot = writeClientSnapshot<ProjectNavBadgeSnapshot>(
    queryClient,
    queryKey,
    NAV_BADGE_SNAPSHOT_STORAGE_PREFIX,
    sessionKey,
    (await getProjectNavBadgeSnapshotCmd({
      projectId,
      url,
      forceRefresh: options?.forceRefresh ?? false,
    })) as unknown as ProjectNavBadgeSnapshot,
    slimNavBadgeSnapshotForSession,
  );
  return {
    ...snapshot,
    signals: mergePrimedCodeScanForAccess(snapshot.signals, false),
  };
}

export async function getAllProjectsWorkSummary(
  forceRefresh = false,
): Promise<TodayProjectWorkSummary[]> {
  return (await getAllProjectsWorkSummaryCmd({
    forceRefresh,
  })) as unknown as TodayProjectWorkSummary[];
}

export function clearProjectSignalSnapshotCache(
  queryClient: QueryClient,
  projectId: number,
  url?: string | null,
) {
  const cacheKey = snapshotCacheKey(projectId, url);
  const normalizedUrl = normalizeAppUrlForKey(url);
  queryClient.removeQueries({
    queryKey: queryKeys.projectSummary.snapshot(projectId, normalizedUrl),
    exact: true,
  });
  queryClient.removeQueries({
    queryKey: queryKeys.projectSummary.navBadge(projectId, normalizedUrl),
    exact: true,
  });
  clearReferenceSignalsCacheKey(queryClient, projectId, url ?? null, false);
  clearReferenceSignalsCacheKey(queryClient, projectId, url ?? null, true);
  clearSessionSnapshotCache(DASHBOARD_SNAPSHOT_STORAGE_PREFIX, cacheKey);
  clearSessionSnapshotCache(NAV_BADGE_SNAPSHOT_STORAGE_PREFIX, cacheKey);
}

export function invalidateProjectSignalSnapshot(
  queryClient: QueryClient,
  projectId: number,
  url?: string | null,
) {
  clearProjectSignalSnapshotCache(queryClient, projectId, url);
  void invalidateProjectSignalSnapshotCmd({
    projectId,
    url: url ?? null,
  }).catch(() => {});
}

export function invalidateProjectMonitoringSignals(
  queryClient: QueryClient,
  projectId: number,
  url?: string | null,
) {
  invalidateProjectSignalSnapshot(queryClient, projectId, url);
}
