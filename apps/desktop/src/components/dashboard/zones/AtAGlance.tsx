import { ScoreRing } from "@/components/ui/score-ring";
import { Button } from "@/components/ui/button";
import { type ScoreBreakdownDisplay } from "@/lib/score-breakdown";

interface SiteScoreData {
  value: number;
  delta: number | null;
  issueCount: number;
  criticalCount: number;
  scanAgeLabel: string | null;
  breakdown: ScoreBreakdownDisplay;
}

interface LastCheckedData {
  label: string;
  kind: "web" | "code";
  sub: string | null;
  stale: boolean;
}

interface UptimeData {
  ratio: number;
  avgResponseMs: number | null;
  outageCount: number;
}

interface VisitorsData {
  visitors: number;
  pageviews: number;
  bouncePct: number | null;
  deltaPct: number | null;
}

interface SeoClicksData {
  clicks: number;
  impressions: number;
  avgPosition: number | null;
  deltaPct: number | null;
}

interface Props {
  siteScore: SiteScoreData | null;
  lastChecked: LastCheckedData | null;
  uptime: UptimeData | null;
  uptimeConfigured?: boolean;
  uptimeLoading?: boolean;
  visitors: VisitorsData | null;
  analyticsConfigured?: boolean;
  analyticsLoading?: boolean;
  seoClicks: SeoClicksData | null;
  searchConfigured?: boolean;
  searchLoading?: boolean;
  onOpenIssues: () => void;
  onRunScan?: () => void;
  onOpenUptime: () => void;
  onOpenAnalytics: () => void;
  onOpenSearchConsole: () => void;
  onOpenIntegrations: () => void;
}

function uptimeColor(ratio: number): string {
  if (ratio >= 99.9) return "text-score-excellent";
  if (ratio >= 99.0) return "text-severity-high";
  return "text-severity-critical";
}

function formatNum(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

interface DeltaProps {
  delta: number;
  higherIsGood?: boolean;
  isPercent?: boolean;
}

function Delta({ delta, higherIsGood = true, isPercent = false }: DeltaProps) {
  if (delta === 0) return null;
  const positive = delta > 0;
  const good = higherIsGood ? positive : !positive;
  const color = good ? "text-score-excellent" : "text-severity-high";
  const triangle = positive ? "▲" : "▼";
  const label = isPercent ? `${Math.abs(delta)}%` : String(Math.abs(delta));
  return (
    <span className={`text-meta glance-delta ${color}`}>
      {triangle} {label}
    </span>
  );
}

interface TileProps {
  label: string;
  onClick: () => void;
  children?: React.ReactNode;
  sub?: string;
  emptyCta?: string;
  emptyMode?: "action" | "connect";
}

interface ScoreTileProps {
  label: string;
  score: number;
  delta: number | null;
  breakdown: ScoreBreakdownDisplay;
  lastCheckedLabel: string | null;
  lastCheckedStale: boolean;
  onClick: () => void;
}

function ScoreTile({
  label,
  score,
  delta,
  breakdown,
  lastCheckedLabel,
  lastCheckedStale,
  onClick,
}: ScoreTileProps) {
  return (
    <Button
      unstyled
      type="button"
      onClick={onClick}
      className="card card--interactive card--compact dashboard-tile">
      <div className="tile__rule">
        <span className="tile__label">
          <span>{label}</span>
        </span>
      </div>
      <div className="glance-score-body">
        <ScoreRing value={score} labelMode="value" size={56} strokeWidth={4} />
        <div className="flex-fill">
          {delta !== null && delta !== 0 ? (
            <div className="glance-delta-row">
              <Delta delta={delta} higherIsGood />
            </div>
          ) : null}
          {breakdown.capNote ? (
            <div className="text-meta glance-meta-line text-severity-critical">Score capped</div>
          ) : null}
          <div
            className={`text-meta glance-meta-line ${lastCheckedStale ? "text-severity-high" : "text-muted-foreground"}`}>
            {lastCheckedLabel ? `Updated ${lastCheckedLabel}` : "Never updated"}
          </div>
        </div>
      </div>
    </Button>
  );
}

function Tile({ label, onClick, children, sub, emptyCta, emptyMode = "action" }: TileProps) {
  const extra = emptyCta ? "glance-tile-empty" : "";
  return (
    <Button
      unstyled
      type="button"
      onClick={onClick}
      className={`card card--interactive card--compact dashboard-tile ${extra}`}>
      <div className="tile__rule">
        <span className="tile__label">
          <span>{label}</span>
        </span>
      </div>
      {emptyCta ? (
        emptyMode === "connect" ? (
          <div className="overview-stat-body">
            <span className="ref-connect-plus text-muted-foreground" aria-hidden="true">
              +
            </span>
            <span className="text-body-muted ref-connect-label">Connect</span>
          </div>
        ) : (
          <span className="tile__cta">{emptyCta}</span>
        )
      ) : (
        <div className="glance-tile-body">
          <div className="glance-value-row">{children}</div>
          {sub && <div className="text-meta glance-tile-sub">{sub}</div>}
        </div>
      )}
    </Button>
  );
}

export function AtAGlance({
  siteScore,
  lastChecked,
  uptime,
  uptimeConfigured = false,
  uptimeLoading = false,
  visitors,
  analyticsConfigured = false,
  analyticsLoading = false,
  seoClicks,
  searchConfigured = false,
  searchLoading = false,
  onOpenIssues,
  onRunScan,
  onOpenUptime,
  onOpenAnalytics,
  onOpenSearchConsole,
  onOpenIntegrations,
}: Props) {
  const uptimeSub =
    uptime && uptime.avgResponseMs !== null ? `${uptime.avgResponseMs}ms avg` : null;

  const visitorsSub = visitors ? `${formatNum(visitors.pageviews)} pageviews` : null;

  const seoSub = seoClicks ? `${formatNum(seoClicks.impressions)} impressions` : null;

  return (
    <div className="glance-grid">
      {siteScore ? (
        <ScoreTile
          label="SiteCMD Score"
          score={siteScore.value}
          delta={siteScore.delta}
          breakdown={siteScore.breakdown}
          lastCheckedLabel={lastChecked?.label ?? null}
          lastCheckedStale={lastChecked?.stale ?? false}
          onClick={onOpenIssues}
        />
      ) : (
        <Tile label="SiteCMD Score" onClick={onRunScan ?? onOpenIssues} emptyCta="Run Scan" />
      )}

      {uptime ? (
        <Tile label="Uptime 30d" onClick={onOpenUptime} sub={uptimeSub ?? undefined}>
          <span className={`glance-tile-value ${uptimeColor(uptime.ratio)}`}>
            {uptime.ratio.toFixed(2)}%
          </span>
        </Tile>
      ) : (
        <Tile
          label="Uptime 30d"
          onClick={!uptimeLoading && !uptimeConfigured ? onOpenIntegrations : onOpenUptime}
          emptyCta={
            uptimeLoading ? "Loading uptime..." : uptimeConfigured ? "View Uptime" : "Connect"
          }
          emptyMode={!uptimeLoading && !uptimeConfigured ? "connect" : "action"}
        />
      )}

      {visitors ? (
        <Tile label="Visitors 30d" onClick={onOpenAnalytics} sub={visitorsSub ?? undefined}>
          <span className="glance-tile-value text-foreground">{formatNum(visitors.visitors)}</span>
          {visitors.deltaPct !== null && visitors.deltaPct !== 0 && (
            <Delta delta={visitors.deltaPct} higherIsGood isPercent />
          )}
        </Tile>
      ) : (
        <Tile
          label="Visitors 30d"
          onClick={!analyticsLoading && !analyticsConfigured ? onOpenIntegrations : onOpenAnalytics}
          emptyCta={
            analyticsLoading
              ? "Loading analytics..."
              : analyticsConfigured
                ? "View Analytics"
                : "Connect"
          }
          emptyMode={!analyticsLoading && !analyticsConfigured ? "connect" : "action"}
        />
      )}

      {seoClicks ? (
        <Tile label="SEO clicks 28d" onClick={onOpenSearchConsole} sub={seoSub ?? undefined}>
          <span className="glance-tile-value text-foreground">{formatNum(seoClicks.clicks)}</span>
          {seoClicks.deltaPct !== null && seoClicks.deltaPct !== 0 && (
            <Delta delta={seoClicks.deltaPct} higherIsGood isPercent />
          )}
        </Tile>
      ) : (
        <Tile
          label="SEO clicks 28d"
          onClick={!searchLoading && !searchConfigured ? onOpenIntegrations : onOpenSearchConsole}
          emptyCta={
            searchLoading ? "Loading search..." : searchConfigured ? "View Search" : "Connect"
          }
          emptyMode={!searchLoading && !searchConfigured ? "connect" : "action"}
        />
      )}
    </div>
  );
}
