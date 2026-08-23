import type { ReactNode } from "react";
import { ProgressBar } from "@/components/ui/progress-bar";
import { FreshnessBadge } from "@/components/ui/surface-meta";
import {
  AlertTriangle,
  BarChart3,
  Eye,
  FileText,
  Monitor,
  MousePointer,
  Search,
  TrendingUp,
} from "lucide-react";
import type {
  BingPageStat,
  BingQueryStat,
  BingSearchData,
  SearchConsoleData,
  SearchDevice,
  SearchPage,
  SearchQuery,
} from "@/lib/analytics-types";
import { formatNum } from "@/lib/tokens";
import { MS_PER_MINUTE } from "@/lib/format";
import {
  PERIODS,
  buildGscObservations,
  type GscObservation,
  type Period,
} from "@/components/dashboard/search-console-page-model";
import { Button } from "@/components/ui/button";
import { TrendChart } from "@/components/dashboard/AnalyticsPageParts";

interface SearchEngineSectionProps {
  title: string;
  data: SearchConsoleData;
  period: Period;
  setPeriod: (p: Period) => void;
  loading: boolean;
  updatedAt: Date | null;
}

export function SearchEngineSection({
  title,
  data,
  period,
  setPeriod,
  loading,
  updatedAt,
}: SearchEngineSectionProps) {
  const { top_queries: topQueries = [], top_pages: topPages = [], daily = [], devices = [] } = data;
  const totalDeviceClicks = devices.reduce((sum, device) => sum + device.clicks, 0) || 1;

  return (
    <section className="card card--spacious">
      <div className="card__title-rule">
        <span className="card__title">
          <Search className="card__icon icon-md" aria-hidden="true" />
          <span>{title}</span>
        </span>
        <div className="row">
          <FreshnessBadge
            loading={loading}
            timestamp={updatedAt}
            prefix="Updated"
            emptyLabel="Waiting for search data"
            staleAfterMs={30 * MS_PER_MINUTE}
          />
          <div className="engine-toggle-shell">
            {PERIODS.map((p) => (
              <Button
                unstyled
                key={p.value}
                type="button"
                onClick={() => setPeriod(p.value)}
                className={`sc-period-btn ${
                  period === p.value ? "sc-period-btn--active" : "sc-period-btn--inactive"
                }`}>
                {p.label}
              </Button>
            ))}
          </div>
        </div>
      </div>
      <p className="sc-section-desc">
        Search demand, click-through, ranking position, and the pages people already find.
      </p>

      <GscObservationsStrip observations={buildGscObservations(data)} />

      <div className="sc-metric-grid">
        <SearchMetricTile
          icon={<MousePointer className="icon-md" />}
          label="Clicks"
          value={formatNum(data.total_clicks)}
          detail="Visits from search results"
        />
        <SearchMetricTile
          icon={<Eye className="icon-md" />}
          label="Impressions"
          value={formatNum(data.total_impressions)}
          detail="Times shown in search"
        />
        <SearchMetricTile
          icon={<TrendingUp className="icon-md" />}
          label="Average CTR"
          value={formatPercent(data.average_ctr)}
          detail="How often results earn clicks"
        />
        <SearchMetricTile
          icon={<Search className="icon-md" />}
          label="Average Position"
          value={formatPosition(data.average_position)}
          detail="Lower is better"
        />
      </div>

      {daily.length > 0 ? <SearchClickTrend daily={daily} /> : null}

      <div className="sc-panels-grid">
        <SearchQueryPanel queries={topQueries} />
        <SearchPagePanel pages={topPages} />
      </div>

      {devices.length > 0 ? (
        <div className="sc-devices">
          <p className="section-label-mid row-tight">
            <Monitor className="icon-sm text-primary" />
            Devices
          </p>
          <div className="sc-devices-grid">
            {devices.map((device) => (
              <DeviceShareRow key={device.device} device={device} totalClicks={totalDeviceClicks} />
            ))}
          </div>
        </div>
      ) : null}
    </section>
  );
}

export function BingSection({
  data,
  loading,
  updatedAt,
}: {
  data: BingSearchData;
  loading: boolean;
  updatedAt: Date | null;
}) {
  return (
    <section className="card card--spacious">
      <div className="card__title-rule">
        <span className="card__title">
          <BarChart3 className="card__icon icon-md" aria-hidden="true" />
          <span>Bing Search Visibility</span>
        </span>
        <div className="row">
          <FreshnessBadge
            loading={loading}
            timestamp={updatedAt}
            prefix="Updated"
            emptyLabel="Waiting for search data"
            staleAfterMs={30 * MS_PER_MINUTE}
          />
        </div>
      </div>
      <p className="sc-section-desc">
        Bing clicks, impressions, ranking position, crawl errors, and visible queries.
      </p>

      <div className="page-split-grid">
        <div>
          <div className="sc-metric-grid">
            <SearchMetricTile
              icon={<MousePointer className="icon-md" />}
              label="Clicks"
              value={formatNum(data.total_clicks)}
              detail="Visits from Bing"
            />
            <SearchMetricTile
              icon={<Eye className="icon-md" />}
              label="Impressions"
              value={formatNum(data.total_impressions)}
              detail="Times shown in Bing"
            />
            <SearchMetricTile
              icon={<Search className="icon-md" />}
              label="Average Position"
              value={formatPosition(data.avg_position)}
              detail="Lower is better"
            />
            <SearchMetricTile
              icon={<AlertTriangle className="icon-md" />}
              label="Crawl Errors"
              value={formatNum(data.crawl_errors)}
              detail={data.crawl_errors > 0 ? "Needs attention" : "No crawl errors"}
              tone={data.crawl_errors > 0 ? "warning" : "success"}
            />
          </div>

          <div className="sc-panels-grid">
            <BingQueryPanel queries={data.top_queries} />
            <BingPagePanel pages={data.top_pages} />
          </div>
        </div>

        <aside>
          <p className="section-label-lg">Bing follow-up</p>
          {data.crawl_errors > 0 ? (
            <div className="sc-bing-alert">
              <p className="eyebrow text-severity-medium">Crawl issue</p>
              <p className="sc-bing-alert-title">
                Bing found {data.crawl_errors} crawl {data.crawl_errors === 1 ? "error" : "errors"}
              </p>
              <p className="text-body-muted sc-bing-alert-body">
                Fix crawl errors so Bing can keep important pages discoverable.
              </p>
            </div>
          ) : (
            <p className="sc-bing-ok">Bing is not reporting crawl errors right now.</p>
          )}
        </aside>
      </div>
    </section>
  );
}

function GscObservationsStrip({ observations }: { observations: GscObservation[] }) {
  if (observations.length === 0) return null;
  return (
    <div className="search-observations-strip">
      <p className="text-micro sc-obs-label">What to look at</p>
      <ul className="stack-snug">
        {observations.map((o) => (
          <li key={o.id} className="text-meta sc-obs-row">
            <span className={`sc-obs-metric ${observationToneClass(o.tone)}`}>{o.metric}</span>
            <span className="min-w-0 text-foreground">
              <span className="sc-obs-strong">{o.label}</span>
              <span className="text-muted-foreground sc-obs-detail">{o.detail}</span>
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function observationToneClass(tone: GscObservation["tone"]): string {
  if (tone === "critical") return "text-severity-critical";
  if (tone === "warning") return "text-severity-medium";
  return "text-primary";
}

function SearchMetricTile({
  detail,
  icon,
  label,
  tone = "info",
  value,
}: {
  detail: string;
  icon: ReactNode;
  label: string;
  tone?: "info" | "success" | "warning";
  value: string;
}) {
  const valueClass = tone === "warning" ? "text-severity-medium" : "text-foreground";

  return (
    <div className="tile">
      <div className="row-tight text-muted-foreground">
        <span className="text-muted-foreground">{icon}</span>
        <span className="section-label-mid">{label}</span>
      </div>
      <p className={`metric-card__value ${valueClass}`}>{value}</p>
      <p className="metric-card__detail">{detail}</p>
    </div>
  );
}

function formatTrendRange(daily: SearchConsoleData["daily"]): string | null {
  if (daily.length === 0) return null;
  const format = (date: string) =>
    new Date(`${date}T00:00:00`).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  const start = format(daily[0].date);
  const end = format(daily[daily.length - 1].date);
  return start === end ? start : `${start} - ${end}`;
}

function SearchClickTrend({ daily }: { daily: SearchConsoleData["daily"] }) {
  const points = daily.map((point) => ({ date: point.date, value: point.clicks }));
  const rangeLabel = formatTrendRange(daily);

  return (
    <div className="sc-trend">
      <div className="row-between sc-trend-head">
        <p className="section-label-mid row-tight">
          <TrendingUp className="icon-sm text-primary" />
          Search clicks
        </p>
        {rangeLabel ? <p className="text-meta">{rangeLabel}</p> : null}
      </div>
      <div className="sc-trend-body">
        <TrendChart points={points} unit="clicks" />
      </div>
    </div>
  );
}

function SearchQueryPanel({ queries }: { queries: SearchQuery[] }) {
  return (
    <div className="compact-surface-panel">
      <p className="section-label-mid row-tight">
        <Search className="icon-sm text-primary" />
        Top queries
      </p>
      <div className="sc-panel-rows">
        {queries.length > 0 ? (
          queries.slice(0, 8).map((query) => <SearchQueryRow key={query.query} query={query} />)
        ) : (
          <p className="text-body-muted sc-panel-empty">No query data yet.</p>
        )}
      </div>
    </div>
  );
}

function SearchPagePanel({ pages }: { pages: SearchPage[] }) {
  return (
    <div className="compact-surface-panel">
      <p className="section-label-mid row-tight">
        <FileText className="icon-sm text-primary" />
        Top pages
      </p>
      <div className="sc-panel-rows">
        {pages.length > 0 ? (
          pages.slice(0, 8).map((page) => <SearchPageRow key={page.page} page={page} />)
        ) : (
          <p className="text-body-muted sc-panel-empty">No page data yet.</p>
        )}
      </div>
    </div>
  );
}

function SearchQueryRow({ query }: { query: SearchQuery }) {
  return (
    <div className="text-body-muted sc-data-row">
      <p className="min-w-0 text-truncate text-foreground sc-data-name">{query.query}</p>
      <p className="text-muted-foreground sc-data-cell">
        {formatNum(query.clicks)} / {formatNum(query.impressions)}
      </p>
      <p className="text-mono text-muted-foreground sc-data-cell">{formatPercent(query.ctr)}</p>
    </div>
  );
}

function SearchPageRow({ page }: { page: SearchPage }) {
  return (
    <div className="text-body-muted sc-data-row">
      <p className="min-w-0 text-truncate text-muted-foreground sc-data-url">{page.page}</p>
      <p className="text-muted-foreground sc-data-cell">
        {formatNum(page.clicks)} / {formatNum(page.impressions)}
      </p>
      <p className="text-mono text-muted-foreground sc-data-cell">{formatPercent(page.ctr)}</p>
    </div>
  );
}

function BingQueryPanel({ queries }: { queries: BingQueryStat[] }) {
  return (
    <div className="compact-surface-panel">
      <p className="section-label-mid row-tight">
        <Search className="icon-sm text-primary" />
        Top Bing queries
      </p>
      <div className="sc-panel-rows">
        {queries.length > 0 ? (
          queries.slice(0, 8).map((query) => (
            <div key={query.query} className="text-body-muted sc-data-row">
              <p className="min-w-0 text-truncate text-foreground sc-data-name">{query.query}</p>
              <p className="text-muted-foreground sc-data-cell">
                {formatNum(query.clicks)} / {formatNum(query.impressions)}
              </p>
              <p className="text-mono text-muted-foreground sc-data-cell">
                {formatPosition(query.avg_position)}
              </p>
            </div>
          ))
        ) : (
          <p className="text-body-muted sc-panel-empty">No Bing query data yet.</p>
        )}
      </div>
    </div>
  );
}

function BingPagePanel({ pages }: { pages: BingPageStat[] }) {
  return (
    <div className="compact-surface-panel">
      <p className="section-label-mid row-tight">
        <FileText className="icon-sm text-primary" />
        Top Bing pages
      </p>
      <div className="sc-panel-rows">
        {pages.length > 0 ? (
          pages.slice(0, 8).map((page) => (
            <div key={page.url} className="text-body-muted sc-data-row">
              <p className="min-w-0 text-truncate text-muted-foreground sc-data-url">{page.url}</p>
              <p className="text-muted-foreground sc-data-cell">
                {formatNum(page.clicks)} / {formatNum(page.impressions)}
              </p>
              <p className="text-mono text-muted-foreground sc-data-cell">
                {formatPosition(page.avg_position)}
              </p>
            </div>
          ))
        ) : (
          <p className="text-body-muted sc-panel-empty">No Bing page data yet.</p>
        )}
      </div>
    </div>
  );
}

function DeviceShareRow({ device, totalClicks }: { device: SearchDevice; totalClicks: number }) {
  return (
    <div>
      <div className="row-between text-body-muted">
        <span className="text-capitalize text-foreground">{device.device}</span>
        <span className="text-mono text-muted-foreground">{formatNum(device.clicks)}</span>
      </div>
      <ProgressBar
        value={(device.clicks / totalClicks) * 100}
        tone="primary"
        label={`${device.device} click share`}
        trackClassName="sc-device-track"
      />
    </div>
  );
}

function formatPercent(value: number | null | undefined): string {
  return `${((value ?? 0) * 100).toFixed(1)}%`;
}

function formatPosition(value: number | null | undefined): string {
  return (value ?? 0).toFixed(1);
}
