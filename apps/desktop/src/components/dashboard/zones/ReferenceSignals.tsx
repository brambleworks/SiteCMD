import { Button } from "@/components/ui/button";
interface WebVitalsData {
  score: number;
  lcpMs: number | null;
  cls: number | null;
  tbtMs: number | null;
}

interface SearchIndexData {
  sourceLabel: string;
  visiblePageCount: number | null;
  totalClicks: number | null;
  totalImpressions: number | null;
}

interface DeliveryData {
  cacheHitPct: number;
  requestsTotal: number;
  threatsBlocked: number;
  bandwidthMb: number;
}

interface DeployReleaseData {
  tagName: string;
  conclusion: string | null;
  ageLabel: string;
  commitsSince: number | null;
}

interface Props {
  webVitals: WebVitalsData | null;
  webVitalsLoading?: boolean;
  searchIndex: SearchIndexData | null;
  searchConfigured?: boolean;
  searchLoading?: boolean;
  delivery: DeliveryData | null;
  deliveryConfigured?: boolean;
  deliveryLoading?: boolean;
  deployRelease: DeployReleaseData | null;
  /** Whether local git history is available without GitHub. */
  deploysFolderLinked?: boolean;
  onOpenWebVitals: () => void;
  onOpenSearchConsole: () => void;
  onOpenDelivery: () => void;
  onOpenDeploys: () => void;
  onOpenIntegrations: () => void;
}

function scoreColor(value: number): string {
  if (value >= 80) return "text-score-excellent";
  if (value >= 50) return "text-severity-high";
  return "text-severity-critical";
}

interface TileProps {
  label: string;
  value?: React.ReactNode;
  sub?: string;
  onClick: () => void;
  emptyCta?: string;
  emptyMode?: "action" | "connect";
}

function RefTile({ label, value, sub, onClick, emptyCta, emptyMode = "action" }: TileProps) {
  return (
    <Button
      unstyled
      type="button"
      onClick={onClick}
      className="card card--interactive dashboard-tile ref-signal-tile">
      <div className="tile__rule">
        <span className="tile__label">
          <span>{label}</span>
        </span>
      </div>
      {emptyCta ? (
        emptyMode === "connect" ? (
          <div className="dashboard-reference-stat">
            <span className="ref-connect-plus text-muted-foreground" aria-hidden="true">
              +
            </span>
            <span className="text-body-muted ref-connect-label">Connect</span>
          </div>
        ) : (
          <span className="tile__cta">{emptyCta}</span>
        )
      ) : (
        <>
          <div className="ref-tile-value">{value}</div>
          {sub && <div className="text-meta ref-tile-sub">{sub}</div>}
        </>
      )}
    </Button>
  );
}

function formatNum(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`;
  return String(n);
}

function formatMs(ms: number): string {
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
  return `${ms}ms`;
}

function buildWebVitalsPresentation(webVitals: WebVitalsData): {
  value: React.ReactNode;
  sub?: string;
} {
  const lcpLabel = webVitals.lcpMs !== null ? `LCP ${formatMs(webVitals.lcpMs)}` : null;
  const clsLabel = webVitals.cls !== null ? `CLS ${webVitals.cls.toFixed(2)}` : null;
  const detailParts = [
    lcpLabel,
    clsLabel,
    webVitals.tbtMs !== null ? `TBT ${webVitals.tbtMs}ms` : null,
    `Score ${webVitals.score}/100`,
  ].filter(Boolean) as string[];

  if (lcpLabel) {
    return {
      value: <span className={scoreColor(webVitals.score)}>{lcpLabel}</span>,
      sub: detailParts.filter((part) => part !== lcpLabel).join(" · "),
    };
  }

  if (clsLabel) {
    return {
      value: <span className={scoreColor(webVitals.score)}>{clsLabel}</span>,
      sub: detailParts.filter((part) => part !== clsLabel).join(" · "),
    };
  }

  return {
    value: (
      <span className={scoreColor(webVitals.score)}>{`Performance ${webVitals.score}/100`}</span>
    ),
    sub: "PageSpeed (mobile)",
  };
}

export function ReferenceSignals({
  webVitals,
  webVitalsLoading = false,
  searchIndex,
  searchConfigured = false,
  searchLoading = false,
  delivery,
  deliveryConfigured = false,
  deliveryLoading = false,
  deployRelease,
  deploysFolderLinked = false,
  onOpenWebVitals,
  onOpenSearchConsole,
  onOpenDelivery,
  onOpenDeploys,
  onOpenIntegrations,
}: Props) {
  const webVitalsPresentation = webVitals ? buildWebVitalsPresentation(webVitals) : null;

  const searchValue = searchIndex
    ? searchIndex.visiblePageCount !== null && searchIndex.visiblePageCount > 0
      ? `${searchIndex.visiblePageCount} visible pages`
      : "Search connected"
    : null;

  const searchSub = searchIndex
    ? [
        searchIndex.sourceLabel,
        searchIndex.totalImpressions !== null
          ? `${formatNum(searchIndex.totalImpressions)} impressions`
          : null,
      ]
        .filter(Boolean)
        .join(" · ")
    : undefined;

  const deliverySub = delivery
    ? [
        `${formatNum(delivery.requestsTotal)} req`,
        delivery.threatsBlocked === 0 ? "0 threats" : `${delivery.threatsBlocked} threats`,
        delivery.bandwidthMb > 0 ? `${delivery.bandwidthMb.toFixed(0)}MB saved` : null,
      ]
        .filter(Boolean)
        .join(" · ")
    : undefined;

  const deploySub = deployRelease
    ? [
        `${deployRelease.ageLabel}`,
        deployRelease.commitsSince !== null && deployRelease.commitsSince > 0
          ? `${deployRelease.commitsSince} commit${deployRelease.commitsSince === 1 ? "" : "s"} since`
          : null,
      ]
        .filter(Boolean)
        .join(" · ")
    : undefined;

  const deployValueColor = deployRelease
    ? deployRelease.conclusion === "success"
      ? "text-score-excellent"
      : deployRelease.conclusion === "failure"
        ? "text-severity-critical"
        : "text-foreground"
    : "text-foreground";

  return (
    <div className="reference-signals-grid">
      {webVitals ? (
        <RefTile
          label="Web Vitals"
          value={webVitalsPresentation?.value}
          sub={webVitalsPresentation?.sub}
          onClick={onOpenWebVitals}
        />
      ) : webVitalsLoading ? (
        <RefTile label="Web Vitals" emptyCta="Loading PageSpeed..." onClick={onOpenWebVitals} />
      ) : (
        <RefTile label="Web Vitals" emptyCta="Run PageSpeed" onClick={onOpenWebVitals} />
      )}

      {searchIndex ? (
        <RefTile
          label="Search & Index"
          value={searchValue}
          sub={searchSub}
          onClick={onOpenSearchConsole}
        />
      ) : (
        <RefTile
          label="Search & Index"
          emptyCta={
            searchLoading ? "Loading search..." : searchConfigured ? "View Search" : "Connect"
          }
          emptyMode={!searchLoading && !searchConfigured ? "connect" : "action"}
          onClick={!searchLoading && !searchConfigured ? onOpenIntegrations : onOpenSearchConsole}
        />
      )}

      {delivery ? (
        <RefTile
          label="Delivery"
          value={`${delivery.cacheHitPct.toFixed(0)}% cache hit`}
          sub={deliverySub}
          onClick={onOpenDelivery}
        />
      ) : (
        <RefTile
          label="Delivery"
          emptyCta={
            deliveryLoading
              ? "Loading delivery..."
              : deliveryConfigured
                ? "View Delivery"
                : "Connect"
          }
          emptyMode={!deliveryLoading && !deliveryConfigured ? "connect" : "action"}
          onClick={!deliveryLoading && !deliveryConfigured ? onOpenIntegrations : onOpenDelivery}
        />
      )}

      {deployRelease ? (
        <RefTile
          label="Deploy & Release"
          value={
            <span className={deployValueColor}>
              {deployRelease.tagName}{" "}
              {deployRelease.conclusion === "success"
                ? "passed"
                : deployRelease.conclusion === "failure"
                  ? "failed"
                  : (deployRelease.conclusion ?? "")}
            </span>
          }
          sub={deploySub}
          onClick={onOpenDeploys}
        />
      ) : deploysFolderLinked ? (
        // A linked folder provides local deploy history without GitHub.
        <RefTile
          label="Deploy & Release"
          emptyCta="View deploys"
          emptyMode="action"
          onClick={onOpenDeploys}
        />
      ) : (
        <RefTile
          label="Deploy & Release"
          emptyCta="Connect"
          emptyMode="connect"
          onClick={onOpenIntegrations}
        />
      )}
    </div>
  );
}
