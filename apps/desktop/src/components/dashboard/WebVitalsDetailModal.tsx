import { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Monitor, RefreshCw, Smartphone, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ExtLink } from "@/components/ui/external-link";
import {
  fetchPageSpeedReport,
  isRateLimitError,
  pageSpeedApiKeyIsSet,
  ratePerformanceScore,
  rateVital,
  ratingColorClass,
  setPageSpeedApiKey,
  type PageSpeedReport,
  type PageSpeedStrategy,
  type VitalMetric,
  type VitalRating,
} from "@/lib/pagespeed";

interface Props {
  url: string;
  hostname: string;
  onClose: () => void;
}

function formatMs(ms: number): string {
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.round(ms)}ms`;
}

const RATING_LABEL: Record<VitalRating, string> = {
  good: "Good",
  "needs-improvement": "Needs work",
  poor: "Poor",
};

interface MetricSpec {
  metric: VitalMetric;
  name: string;
  value: number | null;
  format: (v: number) => string;
  help: string;
}

function labMetrics(report: PageSpeedReport): MetricSpec[] {
  const cls = (v: number) => v.toFixed(2);
  return [
    { metric: "lcp", name: "LCP", value: report.lcpMs, format: formatMs, help: "≤ 2.5s" },
    { metric: "cls", name: "CLS", value: report.cls, format: cls, help: "≤ 0.1" },
    { metric: "tbt", name: "TBT", value: report.tbtMs, format: formatMs, help: "≤ 200ms" },
    { metric: "fcp", name: "FCP", value: report.fcpMs, format: formatMs, help: "≤ 1.8s" },
    { metric: "ttfb", name: "TTFB", value: report.ttfbMs, format: formatMs, help: "≤ 0.8s" },
    { metric: "si", name: "Speed Index", value: report.siMs, format: formatMs, help: "≤ 3.4s" },
  ];
}

function fieldMetrics(report: PageSpeedReport): MetricSpec[] {
  const cls = (v: number) => v.toFixed(2);
  return [
    { metric: "lcp", name: "LCP", value: report.fieldLcpMs, format: formatMs, help: "≤ 2.5s" },
    { metric: "cls", name: "CLS", value: report.fieldCls, format: cls, help: "≤ 0.1" },
    { metric: "inp", name: "INP", value: report.fieldInpMs, format: formatMs, help: "≤ 200ms" },
  ];
}

function MetricCell({ spec }: { spec: MetricSpec }) {
  const rating = rateVital(spec.metric, spec.value);
  return (
    <div className="tile">
      <span className="tile__label">
        <span>{spec.name}</span>
      </span>
      <span className={`vitals-metric-value ${ratingColorClass(rating)}`}>
        {spec.value !== null ? spec.format(spec.value) : "--"}
      </span>
      <span className="text-meta vitals-metric-help">
        {rating ? `${RATING_LABEL[rating]} · ${spec.help}` : spec.help}
      </span>
    </div>
  );
}

function ReportView({ report }: { report: PageSpeedReport }) {
  const scoreRating = ratePerformanceScore(report.performanceScore);
  const field = fieldMetrics(report);
  const hasField = field.some((m) => m.value !== null);
  const fieldSource =
    report.fieldSource === "origin"
      ? "origin-level (whole site)"
      : report.fieldSource === "url"
        ? "this page"
        : null;
  const opportunities = report.opportunities.slice(0, 5);

  return (
    <>
      <div className="vitals-score-row">
        <div className={`vitals-score ${ratingColorClass(scoreRating)}`}>
          {report.performanceScore}
        </div>
        <div className="min-w-0">
          <div className="text-body vitals-heading">Performance {RATING_LABEL[scoreRating]}</div>
          <div className="text-meta">Lighthouse score (0-100), simulated {report.strategy}</div>
        </div>
      </div>

      <p className="text-meta">
        This is a Lighthouse performance and best-practices score, not your SiteCMD score. Your
        SiteCMD score grades overall health across security, performance, SEO, Accessibility, and
        compliance, so the two numbers can differ without either being wrong.
      </p>

      <section>
        <h4 className="vitals-section-label">Lab data · Lighthouse</h4>
        <p className="text-meta vitals-section-desc">
          Simulated load in a controlled environment. Good for diagnosing; numbers vary from
          real-world.
        </p>
        <div className="vitals-metric-grid">
          {labMetrics(report).map((spec) => (
            <MetricCell key={spec.name} spec={spec} />
          ))}
        </div>
      </section>

      <section>
        <h4 className="vitals-section-label">Real-user data · CrUX (28-day)</h4>
        {hasField ? (
          <>
            <p className="text-meta vitals-section-desc">
              Field data from real Chrome visitors{fieldSource ? `, ${fieldSource}` : ""}.
            </p>
            <div className="vitals-metric-grid">
              {field.map((spec) => (
                <MetricCell key={spec.name} spec={spec} />
              ))}
            </div>
          </>
        ) : (
          <p className="text-body-muted">
            No real-user field data available for this URL yet. CrUX needs enough Chrome traffic to
            report.
          </p>
        )}
      </section>

      {opportunities.length > 0 && (
        <section>
          <h4 className="vitals-section-label">Top opportunities</h4>
          <ul className="vitals-opportunity-list">
            {opportunities.map((op) => (
              <li key={op.id} className="vitals-opportunity-row">
                <span className="min-w-0 text-truncate text-body text-foreground">{op.title}</span>
                {op.savingsMs !== null && op.savingsMs > 0 && (
                  <span className="text-meta no-shrink tabular-nums">
                    ~{formatMs(op.savingsMs)}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      <p className="text-meta">Source: Google PageSpeed Insights (Lighthouse v5).</p>
    </>
  );
}

export function WebVitalsDetailModal({ url, hostname, onClose }: Props) {
  const [strategy, setStrategy] = useState<PageSpeedStrategy>("mobile");
  const [report, setReport] = useState<PageSpeedReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [hasKey, setHasKey] = useState(false);
  const [keyInput, setKeyInput] = useState("");
  const [savingKey, setSavingKey] = useState(false);

  const load = useCallback(
    async (target: PageSpeedStrategy) => {
      setLoading(true);
      setError(null);
      try {
        setReport(await fetchPageSpeedReport(url, target));
      } catch (err) {
        setReport(null);
        setError(
          typeof err === "string"
            ? err
            : err instanceof Error
              ? err.message
              : "PageSpeed request failed",
        );
      } finally {
        setLoading(false);
      }
    },
    [url],
  );

  const saveKeyAndRetry = useCallback(async () => {
    const trimmed = keyInput.trim();
    if (!trimmed || savingKey) return;
    setSavingKey(true);
    try {
      await setPageSpeedApiKey(trimmed);
      setHasKey(true);
      setKeyInput("");
      await load(strategy);
    } catch (err) {
      setError(typeof err === "string" ? err : "Could not save the API key");
    } finally {
      setSavingKey(false);
    }
  }, [keyInput, savingKey, load, strategy]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- kicks off the async Web Vitals load for the current strategy
    void load(strategy);
  }, [load, strategy]);

  useEffect(() => {
    let cancelled = false;
    void pageSpeedApiKeyIsSet()
      .then((set) => {
        if (!cancelled) setHasKey(set);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [onClose]);

  return createPortal(
    <div className="fix-prompt-modal-backdrop" onClick={onClose}>
      <section
        className="fix-prompt-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="web-vitals-title"
        onClick={(e) => e.stopPropagation()}>
        <div className="fix-prompt-modal-header">
          <div className="min-w-0">
            <h3 id="web-vitals-title" className="fix-prompt-modal-title">
              Web Vitals
            </h3>
            <p className="text-meta text-truncate vitals-modal-subtitle">
              {hostname} · Google PageSpeed Insights
            </p>
          </div>
          <div className="row no-shrink">
            <div className="toggle-group" role="group" aria-label="PageSpeed strategy">
              <Button
                unstyled
                type="button"
                className={`toggle-btn ${strategy === "mobile" ? "toggle-btn-active" : "toggle-btn-inactive"}`}
                aria-pressed={strategy === "mobile"}
                onClick={() => setStrategy("mobile")}>
                <Smartphone className="icon-sm vitals-toggle-icon" />
                Mobile
              </Button>
              <Button
                unstyled
                type="button"
                className={`toggle-btn ${strategy === "desktop" ? "toggle-btn-active" : "toggle-btn-inactive"}`}
                aria-pressed={strategy === "desktop"}
                onClick={() => setStrategy("desktop")}>
                <Monitor className="icon-sm vitals-toggle-icon" />
                Desktop
              </Button>
            </div>
            <Button
              unstyled
              type="button"
              className="details-close"
              aria-label="Close"
              onClick={onClose}>
              <X />
            </Button>
          </div>
        </div>

        <div className="agent-handoff-body">
          {loading ? (
            <div className="vitals-modal-status">
              <RefreshCw
                className="icon-lg animate-spin text-muted-foreground"
                aria-hidden="true"
              />
              <p className="text-body-muted">Running PageSpeed Insights for {hostname}...</p>
            </div>
          ) : error ? (
            <div className="vitals-modal-status">
              <p className="text-body vitals-heading">Couldn&apos;t load PageSpeed</p>
              <p className="text-body-muted">{error}</p>
              {isRateLimitError(error) ? (
                <div className="vitals-key-prompt">
                  <p className="text-meta">
                    {hasKey
                      ? "A PageSpeed API key is saved, but the shared limit was still hit. Wait a minute and retry, or replace the key below."
                      : "The keyless PageSpeed API is shared and rate-limited. Add a free API key (25,000 runs/day) to fix this - it is stored in your OS keychain."}
                  </p>
                  <div className="vitals-key-row">
                    <input
                      type="password"
                      autoComplete="off"
                      aria-label="PageSpeed API key"
                      value={keyInput}
                      onChange={(event) => setKeyInput(event.target.value)}
                      placeholder={hasKey ? "Paste a new key" : "Paste your PageSpeed API key"}
                      className="field-control field-control--card"
                      disabled={savingKey}
                    />
                    <Button
                      type="button"
                      onClick={saveKeyAndRetry}
                      disabled={!keyInput.trim() || savingKey}>
                      {savingKey ? "Saving..." : "Save & retry"}
                    </Button>
                  </div>
                  <ExtLink
                    href="https://developers.google.com/speed/docs/insights/v5/get-started#APIKey"
                    className="vitals-key-link">
                    Get a free key →
                  </ExtLink>
                </div>
              ) : (
                <p className="text-meta">
                  PageSpeed only works on publicly reachable URLs (not localhost or private
                  networks).
                </p>
              )}
              <Button variant="outline" type="button" onClick={() => load(strategy)}>
                Try again
              </Button>
            </div>
          ) : report ? (
            <ReportView report={report} />
          ) : null}
        </div>

        <div className="fix-prompt-modal-footer">
          <Button
            variant="outline"
            type="button"
            onClick={() => load(strategy)}
            disabled={loading}
            className="modal-footer-lead">
            <RefreshCw className={`icon-md ${loading ? "animate-spin" : ""}`} />
            Refresh
          </Button>
          <Button variant="outline" type="button" onClick={onClose}>
            Close
          </Button>
        </div>
      </section>
    </div>,
    document.body,
  );
}
