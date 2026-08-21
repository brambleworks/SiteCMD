import type { QueryClient } from "@tanstack/react-query";
import { useQuery } from "@tanstack/react-query";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { clearSessionSnapshotCache, parseSnapshotCacheEntry } from "@/lib/project-summary-cache";
import { checkSsl } from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import type { SslProbeResult } from "@/lib/dashboard/types";

const SSL_PROBE_STORAGE_PREFIX = "sitecmd:dashboard-ssl-probe:";
const SSL_PROBE_CACHE_TTL_MS = 30 * 60 * 1000;

interface SslProbeCacheEntry {
  probe: SslProbeResult | null;
  cachedAt: number;
}

function sslProbeCacheKey(url: string) {
  return normalizeAppUrlForKey(url);
}

// sessionStorage persists the certificate chip across webview reloads.
function readSessionSslProbeCache(key: string): SslProbeCacheEntry | null {
  if (typeof window === "undefined") return null;
  const storageKey = `${SSL_PROBE_STORAGE_PREFIX}${key}`;
  try {
    const raw = window.sessionStorage.getItem(storageKey);
    if (!raw) return null;
    const parsed = parseSnapshotCacheEntry<SslProbeResult | null>(JSON.parse(raw) as unknown);
    if (!parsed) {
      window.sessionStorage.removeItem(storageKey);
      return null;
    }
    return { probe: parsed.snapshot, cachedAt: parsed.cachedAt };
  } catch {
    window.sessionStorage.removeItem(storageKey);
    return null;
  }
}

function writeSessionSslProbeCache(key: string, probe: SslProbeResult | null) {
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage.setItem(
      `${SSL_PROBE_STORAGE_PREFIX}${key}`,
      JSON.stringify({ snapshot: probe, cachedAt: Date.now() }),
    );
  } catch {
    // best effort only
  }
}

/** A persisted entry, TTL-gated. `undefined` means "seed nothing" (force a probe). */
function readFreshSessionSslProbeCache(key: string): SslProbeCacheEntry | undefined {
  const cached = readSessionSslProbeCache(key);
  if (!cached) return undefined;
  if (Date.now() - cached.cachedAt > SSL_PROBE_CACHE_TTL_MS) {
    clearSessionSnapshotCache(SSL_PROBE_STORAGE_PREFIX, key);
    return undefined;
  }
  return cached;
}

function failedSslProbe(): SslProbeResult {
  return {
    days_remaining: null,
    auto_renew_hint: false,
    not_after_iso: null,
    error: "Probe failed",
  };
}

/** Drop the cached probe and mark it stale for the next active observer. */
export function invalidateDashboardSslProbe(queryClient: QueryClient, url: string) {
  const key = sslProbeCacheKey(url);
  clearSessionSnapshotCache(SSL_PROBE_STORAGE_PREFIX, key);
  void queryClient.invalidateQueries({ queryKey: queryKeys.sslProbe.forUrl(key) });
}

export function useDashboardSslProbe({
  auxiliarySignalsArmed,
  includeReferenceSignals,
  url,
}: {
  auxiliarySignalsArmed: boolean;
  includeReferenceSignals: boolean;
  url: string;
}): SslProbeResult | null {
  const key = sslProbeCacheKey(url);
  const { data } = useQuery({
    queryKey: queryKeys.sslProbe.forUrl(key),
    queryFn: () =>
      checkSsl({ url })
        .catch(() => failedSslProbe())
        .then((probe) => {
          writeSessionSslProbeCache(key, probe);
          return probe;
        }),
    // Delay probing until auxiliary signals arm while retaining fresh cached data.
    enabled: includeReferenceSignals && auxiliarySignalsArmed,
    staleTime: SSL_PROBE_CACHE_TTL_MS,
    gcTime: SSL_PROBE_CACHE_TTL_MS,
    initialData: () => readFreshSessionSslProbeCache(key)?.probe,
    initialDataUpdatedAt: () => readFreshSessionSslProbeCache(key)?.cachedAt,
  });

  if (!includeReferenceSignals) return null;
  return data ?? null;
}
