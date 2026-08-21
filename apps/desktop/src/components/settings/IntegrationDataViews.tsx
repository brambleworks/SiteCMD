import { Eye, Clock, Wifi, WifiOff } from "lucide-react";
import { formatNum, formatBytes, formatDuration } from "@/lib/tokens";

/** Plausible Analytics - visitors, pageviews, bounce rate, top pages & sources. */
export function PlausibleDataView({ data }: { data: Record<string, unknown> }) {
  const topPages = (data.top_pages as Array<{ page: string; visitors: number }>) || [];
  const topSources = (data.top_sources as Array<{ source: string; visitors: number }>) || [];

  return (
    <div className="stack-base">
      <div className="integration-stat-grid">
        <Stat
          label="Visitors"
          value={formatNum(data.visitors as number)}
          icon={<Eye className="icon-xs" />}
        />
        <Stat label="Pageviews" value={formatNum(data.pageviews as number)} />
        <Stat label="Bounce Rate" value={`${((data.bounce_rate as number) || 0).toFixed(0)}%`} />
        <Stat
          label="Avg Duration"
          value={formatDuration(data.visit_duration as number)}
          icon={<Clock className="icon-xs" />}
        />
      </div>
      <div className="integration-stat-grid integration-stat-grid--2">
        {topPages.length > 0 && (
          <div>
            <p className="section-label integration-top-label">Top Pages</p>
            {topPages.slice(0, 5).map((p) => (
              <div key={p.page} className="integration-top-row">
                <span className="integration-top-name">{p.page}</span>
                <span className="tabular-nums text-muted-foreground">{formatNum(p.visitors)}</span>
              </div>
            ))}
          </div>
        )}
        {topSources.length > 0 && (
          <div>
            <p className="section-label integration-top-label">Top Sources</p>
            {topSources.slice(0, 5).map((s) => (
              <div key={s.source} className="integration-top-row">
                <span className="integration-top-name">{s.source}</span>
                <span className="tabular-nums text-muted-foreground">{formatNum(s.visitors)}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/** Cloudflare - cache hit rate, requests, bandwidth, threats. */
export function CloudflareDataView({ data }: { data: Record<string, unknown> }) {
  return (
    <div className="integration-stat-grid">
      <Stat
        label="Cache Hit Rate"
        value={`${((data.cache_hit_rate as number) || 0).toFixed(0)}%`}
      />
      <Stat label="Requests" value={formatNum(data.requests_total as number)} />
      <Stat label="Bandwidth" value={formatBytes(data.bandwidth_total as number)} />
      <Stat
        label="Threats"
        value={String(data.threats_blocked || 0)}
        highlight={(data.threats_blocked as number) > 0}
      />
    </div>
  );
}

/** UptimeRobot - status, uptime ratio, avg response time. */
export function UptimeRobotDataView({ data }: { data: Record<string, unknown> }) {
  const monitors = (data.monitors as Array<Record<string, unknown>>) || [];
  const primary = monitors[0];
  if (!primary) return <p className="muted-text">No monitors found matching this site.</p>;

  const isUp = (primary.status as number) === 2;

  return (
    <div className="stack-snug">
      <div className="row-loose">
        {isUp ? (
          <Wifi className="icon-md text-score-excellent" />
        ) : (
          <WifiOff className="icon-md text-severity-critical" />
        )}
        <span
          className={`integration-status-text ${isUp ? "text-score-excellent" : "text-severity-critical"}`}>
          {primary.status_text as string}
        </span>
      </div>
      <div className="integration-stat-grid integration-stat-grid--3">
        <Stat label="Uptime" value={`${((primary.uptime_ratio as number) || 0).toFixed(2)}%`} />
        <Stat
          label="Avg Response"
          value={`${primary.average_response as number}ms`}
          icon={<Clock className="icon-xs" />}
        />
        <Stat label="Monitor" value={(primary.friendly_name as string) || "-"} />
      </div>
    </div>
  );
}

export function GenericIntegrationDataView({ data }: { data: Record<string, unknown> }) {
  const status = typeof data.status_text === "string" ? data.status_text : "Configured";
  const target = typeof data.target === "string" ? data.target : null;
  const providerUrl = typeof data.provider_url === "string" ? data.provider_url : null;
  const mode = typeof data.mode === "string" ? data.mode : null;

  return (
    <div className="stack-snug">
      <div className="integration-stat-grid integration-stat-grid--3">
        <Stat label="Status" value={status} />
        <Stat label="Target" value={target ?? "Saved"} />
        <Stat label="Mode" value={mode ?? "Connected"} />
      </div>
      {providerUrl ? (
        <p className="muted-text">
          Source: <span className="text-foreground">{providerUrl}</span>
        </p>
      ) : null}
    </div>
  );
}

/** Generic stat display - label + value with optional icon and highlight. */
function Stat({
  label,
  value,
  icon,
  highlight,
}: {
  label: string;
  value: string;
  icon?: React.ReactNode;
  highlight?: boolean;
}) {
  return (
    <div>
      <p className="subtitle-xs integration-stat-label">{label}</p>
      <p className={`integration-stat-value ${highlight ? "text-severity-medium" : ""}`}>
        {icon}
        {value}
      </p>
    </div>
  );
}
