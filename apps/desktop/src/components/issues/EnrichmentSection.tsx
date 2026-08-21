import type { Enrichment } from "@/lib/types";
import { DossierNumberedSection } from "@/components/issues/IssueDossierPanel";

interface Props {
  enrichments: Enrichment[];
}

function renderDetail(e: Enrichment): string {
  switch (e.kind) {
    case "fieldLcp":
      return `Real-user LCP p75: ${(e.p75_ms / 1000).toFixed(1)}s on ${e.url}`;
    case "fieldCls":
      return `Real-user CLS: ${e.value.toFixed(2)} on ${e.url}`;
    case "fieldInp":
      return `Real-user INP p75: ${e.p75_ms}ms on ${e.url}`;
    case "searchImpressionsDrop":
      return `Search impressions dropped from ${e.from} to ${e.to} over ${e.days}d`;
    case "recentCrawlErrors":
      return `${e.count} crawl errors in the last ${e.days}d`;
    case "recentDowntime":
      return `Downtime from ${e.window_start} to ${e.window_end}`;
    case "certExpiresIn":
      return e.days <= 0 ? "Cert expired" : `Cert expires in ${e.days}d`;
    case "certChain":
      return `Cert chain: ${e.issues.join(", ")}`;
    case "ttfbHistory":
      return `TTFB p75: ${e.p75_ms}ms over ${e.days}d`;
    case "botTrafficPct":
      return `Bot traffic: ${(e.value * 100).toFixed(0)}%`;
    case "cacheHitRate":
      return `Cache hit rate: ${(e.value * 100).toFixed(0)}%`;
    case "recentFiveXxSpike":
      return `5xx rate spiked to ${(e.rate * 100).toFixed(1)}% at ${e.started_at}`;
    case "recentOriginErrors":
      return `${e.count} origin errors in the last ${e.days}d`;
    case "topFallingPage":
      return `${e.url} traffic down ${e.pct_drop.toFixed(0)}%`;
    case "topFallingFunnel":
      return `Funnel "${e.name}" down ${e.pct_drop.toFixed(0)}%`;
  }
}

function sourceLabel(s: string): string {
  switch (s) {
    case "gsc":
      return "Google Search Console";
    case "uptimerobot":
      return "UptimeRobot";
    case "cloudflare":
      return "Cloudflare";
    case "plausible":
      return "Plausible";
    default:
      return s;
  }
}

export function EnrichmentSection({ enrichments }: Props) {
  if (enrichments.length === 0) return null;
  return (
    <DossierNumberedSection label="What your integrations say" tone="neutral">
      <ul className="enrichment-list">
        {enrichments.map((e, i) => (
          <li key={i} className="enrichment-row">
            <span className="enrichment-detail">{renderDetail(e)}</span>
            <span className="enrichment-source">via {sourceLabel(e.source)}</span>
          </li>
        ))}
      </ul>
    </DossierNumberedSection>
  );
}
