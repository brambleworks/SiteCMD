import { useState, useEffect, useCallback, useMemo } from "react";
import { buildAnalyticsSnapshotKey } from "@/lib/analytics-snapshot-cache";
import type { NavTarget } from "@/components/layout/nav-page";
import { useAnalyticsQuery } from "./useAnalyticsQuery";
import { HeaderActions } from "@/app/ShellHeader";
import {
  BarChart3,
  Loader2,
  RefreshCw,
  Users,
  Eye,
  ArrowUpRight,
  Timer,
  Globe,
  Monitor,
  Shield,
  Zap,
  HardDrive,
  Activity,
} from "lucide-react";
import type { AnalyticsResponse } from "@/lib/analytics-types";
import { InlineIntegrationSetup } from "@/components/settings/InlineIntegrationSetup";
import { formatNum, formatDuration } from "@/lib/tokens";
import { MS_PER_MINUTE } from "@/lib/format";
import { Button } from "@/components/ui/button";
import { FreshnessBadge } from "@/components/ui/surface-meta";
import { SurfaceState } from "@/components/ui/surface-state";
import { AnalyticsLoadingState } from "./AnalyticsPageLoadingState";
import { TrafficSourcesModal } from "./TrafficSourcesModal";
import { BreakdownCard, MetricCard, TrendChart } from "./AnalyticsPageParts";
import {
  ANALYTICS_PERIODS,
  countryName,
  formatBytes,
  type AnalyticsPeriod,
} from "./analytics-page-model";

interface AnalyticsPageProps {
  projectId: number;
  url: string;
  onNavigate?: (page: NavTarget) => void;
}

export function AnalyticsPage({ projectId, url, onNavigate }: AnalyticsPageProps) {
  return <AnalyticsPageInner projectId={projectId} url={url} onNavigate={onNavigate} />;
}

const ANALYTICS_SNAPSHOT_VARIANT = "traffic";

function buildKey(projectId: number, period: AnalyticsPeriod, url: string): string {
  return buildAnalyticsSnapshotKey(projectId, `${period}:${url}`, ANALYTICS_SNAPSHOT_VARIANT);
}

function AnalyticsPageInner({ projectId, url, onNavigate }: AnalyticsPageProps) {
  const [period, setPeriod] = useState<AnalyticsPeriod>("30d");
  const [syncing, setSyncing] = useState(false);
  const [sourcesOpen, setSourcesOpen] = useState(false);

  // The shared query owns transport and persistence; this page derives traffic views.
  const {
    data,
    fetchedAt: loadedAt,
    isFetching: loading,
    isError,
    refresh: refreshData,
  } = useAnalyticsQuery({
    projectId,
    period,
    siteUrl: url,
    snapshotKey: buildKey(projectId, period, url),
  });

  const merged = useMemo(() => {
    if (!data) return null;
    const p = data.plausible;
    const g = data.google_analytics;
    const cf = data.cloudflare;
    const ur = data.uptimerobot;

    // Traffic: prefer Plausible, fall back to GA4, then Cloudflare
    const visitors = p?.aggregate.visitors ?? g?.active_users ?? cf?.unique_visitors ?? null;
    const pageviews = p?.aggregate.pageviews ?? g?.pageviews ?? cf?.page_views ?? null;
    const bounceRate = p?.aggregate.bounce_rate ?? g?.bounce_rate ?? null;
    const avgDuration = p?.aggregate.visit_duration ?? g?.avg_session_duration ?? null;

    // Trend: prefer Plausible, fall back to GA4
    const trendPoints =
      p?.points?.map((pt) => ({ date: pt.date, value: pt.visitors })) ??
      g?.daily?.map((pt) => ({ date: pt.date, value: pt.users })) ??
      null;
    const trendTotal = trendPoints?.reduce((sum, point) => sum + point.value, 0) ?? null;

    // Top pages: merge Plausible + GA4 (dedup by path)
    const topPages: { page: string; value: number }[] = [];
    const seenPages = new Set<string>();
    for (const pg of p?.top_pages ?? []) {
      if (!seenPages.has(pg.page)) {
        seenPages.add(pg.page);
        topPages.push({ page: pg.page, value: pg.visitors });
      }
    }
    for (const pg of g?.top_pages ?? []) {
      if (!seenPages.has(pg.page)) {
        seenPages.add(pg.page);
        topPages.push({ page: pg.page, value: pg.views });
      }
    }
    topPages.sort((a, b) => b.value - a.value);

    // Sources: merge Plausible + GA4
    const topSources: { source: string; value: number }[] = [];
    const seenSrc = new Set<string>();
    for (const s of p?.top_sources ?? []) {
      const label = s.source || "Direct / None";
      if (!seenSrc.has(label)) {
        seenSrc.add(label);
        topSources.push({ source: label, value: s.visitors });
      }
    }
    for (const s of g?.top_sources ?? []) {
      if (!seenSrc.has(s.source)) {
        seenSrc.add(s.source);
        topSources.push({ source: s.source, value: s.users });
      }
    }
    topSources.sort((a, b) => b.value - a.value);

    const monitor = ur?.monitors?.[0] ?? null;
    const uptimeRatio = monitor?.uptime_ratio ?? null;
    const uptimeStatus = monitor?.status_text ?? null;
    const avgResponse = monitor?.average_response ?? null;

    const cacheHitRate = cf?.cache_hit_rate ?? null;
    const requestsTotal = cf?.requests_total ?? null;
    const bandwidthTotal = cf?.bandwidth_total ?? null;
    const threatsBlocked = cf?.threats_blocked ?? null;

    const countries = p?.countries ?? null;
    const devices = p?.devices ?? null;
    const browsers = p?.browsers ?? null;

    return {
      visitors,
      pageviews,
      bounceRate,
      avgDuration,
      trendPoints,
      trendTotal,
      topPages,
      topSources,
      uptimeRatio,
      uptimeStatus,
      avgResponse,
      monitor,
      cacheHitRate,
      requestsTotal,
      bandwidthTotal,
      threatsBlocked,
      countries,
      devices,
      browsers,
      hasTraffic: visitors !== null,
      hasUptime: uptimeRatio !== null,
      hasCdn: cacheHitRate !== null,
    };
  }, [data]);

  const hasAnyData =
    data?.cloudflare || data?.plausible || data?.uptimerobot || data?.google_analytics;

  const handleConnected = useCallback(() => {
    setSyncing(true);
    void refreshData();
  }, [refreshData]);

  // Poll briefly after connection until the provider's first data arrives.
  useEffect(() => {
    if (!syncing) return;
    if (hasAnyData) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- ends the sync-in-progress flag once the polling loop reports data
      setSyncing(false);
      return;
    }
    const startedAt = Date.now();
    const SYNC_POLL_MS = 3000;
    const SYNC_TIMEOUT_MS = 30000;
    const timer = setInterval(() => {
      if (Date.now() - startedAt > SYNC_TIMEOUT_MS) {
        setSyncing(false);
        return;
      }
      void refreshData();
    }, SYNC_POLL_MS);
    return () => clearInterval(timer);
  }, [syncing, hasAnyData, refreshData]);

  const providerErrors = useMemo(() => getAnalyticsProviderErrors(data), [data]);
  // Configured providers with errors require reconnection.
  const reconnectTypes = useMemo(
    () => providerErrors.map((providerError) => providerError.integrationType),
    [providerErrors],
  );
  const analyticsSources = useMemo(() => getAnalyticsSources(data), [data]);
  const selectedPeriodLabel =
    ANALYTICS_PERIODS.find((range) => range.value === period)?.label ?? "Selected period";

  if (isError && !data) {
    // Keep retry available while showing connect actions for an empty account.
    return (
      <div className="page-content stack-hero">
        <SurfaceState
          kind="error"
          title="Analytics could not load"
          description="We could not pull traffic, uptime, or CDN data right now. Retry in a moment, or connect a provider below."
          primaryAction={{ label: "Retry", onClick: refreshData }}
        />
        <InlineIntegrationSetup
          serviceTypes={["plausible", "googleanalytics", "cloudflare", "uptimerobot"]}
          projectId={projectId}
          url={url}
          includeGoogle
          onConnected={handleConnected}
        />
      </div>
    );
  }

  if ((loading || syncing) && !hasAnyData) {
    return <AnalyticsLoadingState syncing={syncing} />;
  }

  return (
    <div className="page-content stack-hero">
      <HeaderActions>
        <Button
          unstyled
          onClick={refreshData}
          disabled={loading}
          aria-label="Refresh analytics data"
          className="refresh-icon-button">
          {loading ? (
            <Loader2 className="icon-md animate-spin text-muted-foreground" />
          ) : (
            <RefreshCw className="icon-md text-muted-foreground" />
          )}
        </Button>
      </HeaderActions>

      {!loading && !syncing && !hasAnyData && (
        <SurfaceState
          kind="empty"
          icon={<BarChart3 className="empty-state-icon" />}
          title="No analytics data yet"
          description="Connect traffic, uptime, or CDN providers to watch live usage and production stability from one place."
        />
      )}

      {merged && hasAnyData && (
        <>
          <div className="stack-base">
            <FreshnessBadge
              loading={loading}
              timestamp={loadedAt}
              prefix="Updated"
              emptyLabel="Waiting for analytics data"
              staleAfterMs={30 * MS_PER_MINUTE}
            />
            <div className="row-between">
              <div className="segmented-control" aria-label="Analytics date range">
                {ANALYTICS_PERIODS.map((range) => (
                  <Button
                    unstyled
                    key={range.value}
                    type="button"
                    onClick={() => setPeriod(range.value)}
                    className={
                      period === range.value
                        ? "analytics-period-button analytics-period-button--active"
                        : "analytics-period-button"
                    }>
                    {range.label}
                  </Button>
                ))}
              </div>
              <Button
                variant="outline"
                size="sm"
                className="no-shrink"
                aria-haspopup="dialog"
                onClick={() => setSourcesOpen(true)}>
                {providerErrors.length > 0 ? (
                  <Activity className="icon-sm text-severity-medium" aria-hidden="true" />
                ) : null}
                Sources
              </Button>
            </div>
          </div>

          {sourcesOpen ? (
            <TrafficSourcesModal
              sources={analyticsSources}
              providerErrors={providerErrors}
              onNavigate={onNavigate}
              onClose={() => setSourcesOpen(false)}
            />
          ) : null}

          {merged.hasTraffic && (
            <section className="panel panel--flush">
              <div className="analytics-panel-head">
                <div className="card__title-rule">
                  <span className="card__title">
                    <Users className="card__icon icon-md" aria-hidden="true" />
                    <span>Traffic Summary</span>
                  </span>
                </div>
              </div>
              <div className="analytics-metric-grid">
                <MetricCard
                  icon={<Users className="icon-md" />}
                  label="Total Visitors"
                  value={merged.visitors !== null ? formatNum(merged.visitors) : "-"}
                  detail="unique people"
                />
                <MetricCard
                  icon={<Eye className="icon-md" />}
                  label="Pageviews"
                  value={merged.pageviews !== null ? formatNum(merged.pageviews) : "-"}
                  detail="total page loads"
                />
                <MetricCard
                  icon={<ArrowUpRight className="icon-md" />}
                  label="Bounce Rate"
                  value={merged.bounceRate !== null ? `${merged.bounceRate.toFixed(0)}%` : "-"}
                  detail="single-page sessions"
                  tone={merged.bounceRate !== null && merged.bounceRate > 70 ? "warning" : "info"}
                />
                <MetricCard
                  icon={<Timer className="icon-md" />}
                  label="Avg Duration"
                  value={merged.avgDuration !== null ? formatDuration(merged.avgDuration) : "-"}
                  detail="average session time"
                />
              </div>
            </section>
          )}

          {merged.trendPoints && merged.trendPoints.length > 1 ? (
            <section className="panel panel--flush">
              <div className="analytics-panel-head">
                <div className="card__title-rule">
                  <span className="card__title">
                    <BarChart3 className="card__icon icon-md" aria-hidden="true" />
                    <span>Recent Traffic</span>
                  </span>
                  <div className="analytics-trend-total">
                    <p className="analytics-trend-value">{formatNum(merged.trendTotal ?? 0)}</p>
                    <p className="section-label-mid analytics-trend-sub">
                      {selectedPeriodLabel} total
                    </p>
                  </div>
                </div>
              </div>
              <div className="analytics-chart-body">
                <div className="compact-surface-panel">
                  <TrendChart points={merged.trendPoints} />
                </div>
              </div>
            </section>
          ) : null}

          {(merged.topSources.length > 0 || merged.topPages.length > 0) && (
            <div
              className={`analytics-split-grid ${
                merged.topSources.length > 0 && merged.topPages.length > 0
                  ? "analytics-split-grid--2"
                  : ""
              }`}>
              {merged.topSources.length > 0 ? (
                <BreakdownCard
                  title="Traffic Sources"
                  icon={<Users className="icon-md" />}
                  items={merged.topSources.map((s) => ({ label: s.source, value: s.value }))}
                />
              ) : null}
              {merged.topPages.length > 0 ? (
                <BreakdownCard
                  title="Top Pages"
                  icon={<Eye className="icon-md" />}
                  items={merged.topPages.map((p) => ({ label: p.page, value: p.value }))}
                />
              ) : null}
            </div>
          )}

          {merged.hasUptime && merged.monitor ? (
            <section className="panel panel--spacious">
              <div className="analytics-uptime-head">
                <div>
                  <div className="card__title-rule">
                    <span className="card__title">
                      <Monitor className="card__icon icon-md" aria-hidden="true" />
                      <span>Uptime & Reliability</span>
                    </span>
                  </div>
                  <p className="analytics-note">
                    Availability and response time from the primary monitor.
                  </p>
                </div>
                <div className="row">
                  <span
                    className={`analytics-status-dot ${
                      merged.monitor.status === 2
                        ? "analytics-status-dot--up"
                        : "analytics-status-dot--down"
                    }`}
                  />
                  <span
                    className={`analytics-status-text ${
                      merged.monitor.status === 2
                        ? "text-score-excellent"
                        : "text-severity-critical"
                    }`}>
                    {merged.uptimeStatus}
                  </span>
                </div>
              </div>
              <div className="analytics-uptime-grid">
                <MetricCard
                  icon={<Monitor className="icon-md" />}
                  label="Uptime"
                  value={`${merged.uptimeRatio?.toFixed(2)}%`}
                  detail="30-day uptime ratio"
                  tone={merged.monitor.status === 2 ? "success" : "critical"}
                />
                <MetricCard
                  icon={<Timer className="icon-md" />}
                  label="Avg Response"
                  value={merged.avgResponse !== null ? `${merged.avgResponse.toFixed(0)}ms` : "-"}
                  detail="primary monitor"
                />
              </div>
            </section>
          ) : null}

          {merged.hasCdn && (
            <section className="panel panel--flush">
              <div className="analytics-panel-head">
                <div className="card__title-rule">
                  <span className="card__title">
                    <Zap className="card__icon icon-md" aria-hidden="true" />
                    <span>CDN & Caching</span>
                  </span>
                </div>
                <p className="analytics-note">
                  Cache efficiency, CDN request volume, bandwidth, and blocked threats from
                  Cloudflare.
                </p>
              </div>
              <div className="analytics-metric-grid">
                <MetricCard
                  icon={<Zap className="icon-md" />}
                  label="Cache Hit Rate"
                  value={merged.cacheHitRate !== null ? `${merged.cacheHitRate.toFixed(1)}%` : "-"}
                  detail="served from cache"
                  tone={
                    merged.cacheHitRate !== null && merged.cacheHitRate >= 70
                      ? "success"
                      : "warning"
                  }
                />
                <MetricCard
                  icon={<HardDrive className="icon-md" />}
                  label="CDN Requests"
                  value={merged.requestsTotal !== null ? formatNum(merged.requestsTotal) : "-"}
                  detail="requests handled by Cloudflare"
                />
                <MetricCard
                  icon={<Activity className="icon-md" />}
                  label="Bandwidth"
                  value={merged.bandwidthTotal !== null ? formatBytes(merged.bandwidthTotal) : "-"}
                  detail="total transfer"
                />
                <MetricCard
                  icon={<Shield className="icon-md" />}
                  label="Threats Blocked"
                  value={merged.threatsBlocked !== null ? formatNum(merged.threatsBlocked) : "-"}
                  detail="Cloudflare security"
                  tone={
                    merged.threatsBlocked !== null && merged.threatsBlocked > 0
                      ? "warning"
                      : "success"
                  }
                />
              </div>
            </section>
          )}

          {(merged.countries || merged.devices || merged.browsers) && (
            <div className="analytics-geo-grid">
              {merged.countries && merged.countries.length > 0 && (
                <BreakdownCard
                  title="Countries"
                  icon={<Globe className="icon-md" />}
                  items={merged.countries.map((c) => ({
                    label: countryName(c.country),
                    value: c.visitors,
                  }))}
                />
              )}
              {merged.devices && merged.devices.length > 0 && (
                <BreakdownCard
                  title="Devices"
                  icon={<Monitor className="icon-md" />}
                  items={merged.devices.map((d) => ({ label: d.device, value: d.visitors }))}
                />
              )}
              {merged.browsers && merged.browsers.length > 0 && (
                <BreakdownCard
                  title="Browsers"
                  items={merged.browsers.map((b) => ({ label: b.browser, value: b.visitors }))}
                />
              )}
            </div>
          )}
        </>
      )}

      {!loading && !syncing && (
        <InlineIntegrationSetup
          serviceTypes={["plausible", "googleanalytics", "cloudflare", "uptimerobot"]}
          projectId={projectId}
          url={url}
          includeGoogle
          allowReconnect={reconnectTypes}
          onConnected={handleConnected}
        />
      )}
    </div>
  );
}

// Search providers belong to the Search page.
const ANALYTICS_PROVIDER_ERROR_LABELS = {
  plausible_error: "Plausible",
  cloudflare_error: "Cloudflare",
  uptimerobot_error: "UptimeRobot",
  google_analytics_error: "Google Analytics",
} as const;

const ANALYTICS_PROVIDER_INTEGRATION_TYPES = {
  plausible_error: "plausible",
  cloudflare_error: "cloudflare",
  uptimerobot_error: "uptimerobot",
  google_analytics_error: "googleanalytics",
} as const;

type AnalyticsProviderErrorKey = keyof typeof ANALYTICS_PROVIDER_ERROR_LABELS;
type AnalyticsProviderError = {
  label: (typeof ANALYTICS_PROVIDER_ERROR_LABELS)[AnalyticsProviderErrorKey];
  integrationType: (typeof ANALYTICS_PROVIDER_INTEGRATION_TYPES)[AnalyticsProviderErrorKey];
  message: string;
};

function getAnalyticsProviderErrors(data: AnalyticsResponse | null): AnalyticsProviderError[] {
  if (!data) return [];
  const errors: AnalyticsProviderError[] = [];
  for (const key of Object.keys(ANALYTICS_PROVIDER_ERROR_LABELS) as AnalyticsProviderErrorKey[]) {
    const value = data[key];
    if (typeof value !== "string" || value.trim().length === 0) continue;
    errors.push({
      label: ANALYTICS_PROVIDER_ERROR_LABELS[key],
      integrationType: ANALYTICS_PROVIDER_INTEGRATION_TYPES[key],
      message: value,
    });
  }
  return errors;
}

const ANALYTICS_SOURCE_OPTIONS = [
  {
    label: "Plausible",
    integrationType: "plausible",
    dataKey: "plausible",
    errorKey: "plausible_error",
  },
  {
    label: "GA4",
    integrationType: "googleanalytics",
    dataKey: "google_analytics",
    errorKey: "google_analytics_error",
  },
  {
    label: "Cloudflare",
    integrationType: "cloudflare",
    dataKey: "cloudflare",
    errorKey: "cloudflare_error",
  },
  {
    label: "UptimeRobot",
    integrationType: "uptimerobot",
    dataKey: "uptimerobot",
    errorKey: "uptimerobot_error",
  },
] as const;

type AnalyticsSourceStatus = "connected" | "attention" | "missing";

function getAnalyticsSources(data: AnalyticsResponse | null) {
  return ANALYTICS_SOURCE_OPTIONS.map((source) => {
    const hasData = Boolean(data?.[source.dataKey]);
    const hasError = Boolean(data?.[source.errorKey]);
    const status: AnalyticsSourceStatus = hasData
      ? "connected"
      : hasError
        ? "attention"
        : "missing";
    return {
      label: source.label,
      integrationType: source.integrationType,
      status,
      statusLabel:
        status === "connected" ? "Added" : status === "attention" ? "Check setup" : "Not added",
    };
  });
}
