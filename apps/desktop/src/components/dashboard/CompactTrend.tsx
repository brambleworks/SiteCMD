import { useId } from "react";
import type { CompactTrendModel, CompactTrendTone } from "./compact-trend-model";
import { Button } from "@/components/ui/button";

interface CompactTrendStripProps {
  models: CompactTrendModel[];
  className?: string;
}

interface CompactTrendCardProps {
  model: CompactTrendModel;
  onClick?: () => void;
}

interface CompactTrendSparklineProps {
  height?: number;
  model: CompactTrendModel;
}

export function CompactTrendStrip({ className = "", models }: CompactTrendStripProps) {
  if (models.length === 0) return null;

  return (
    <div
      className={`compact-trend-strip ${models.length > 1 ? "compact-trend-strip--multi" : ""} ${className}`}
      aria-label="Trend summary">
      {models.map((model) => (
        <CompactTrendCard key={model.key} model={model} />
      ))}
    </div>
  );
}

function CompactTrendCard({ model, onClick }: CompactTrendCardProps) {
  const content = (
    <>
      <div className="compact-trend-head">
        <div className="flex-fill">
          <p className="section-label-mid text-truncate">{model.label}</p>
          <div className="compact-trend-value-row">
            <span className="compact-trend-value">{model.currentValue}</span>
            <span className="text-body-muted compact-trend-detail">{model.detail}</span>
          </div>
        </div>
        <TrendDeltaBadge tone={model.tone}>{model.deltaLabel}</TrendDeltaBadge>
      </div>
      <div className="compact-trend-spark">
        <CompactTrendSparkline model={model} height={44} />
      </div>
    </>
  );

  if (onClick) {
    return (
      <Button
        unstyled
        type="button"
        onClick={onClick}
        className="card card--interactive compact-trend-card">
        {content}
      </Button>
    );
  }

  return <div className="card compact-trend-card">{content}</div>;
}

export function CompactTrendSparkline({ height = 40, model }: CompactTrendSparklineProps) {
  const gradientId = `compact-trend-${useId().replace(/:/g, "")}`;
  const color = getTrendColor(model.tone);
  const series = model.series;
  const hasTrend = series.length >= 2;
  const w = 360;
  const h = height;
  const padX = 4;
  const padY = 6;

  if (!hasTrend) {
    return (
      <svg
        aria-hidden="true"
        className="compact-trend-svg"
        preserveAspectRatio="none"
        viewBox={`0 0 ${w} ${h}`}>
        <path
          d={`M${padX},${Math.round(h / 2)} L${w - padX},${Math.round(h / 2)}`}
          fill="none"
          stroke="var(--border)"
          strokeDasharray="4 6"
          strokeLinecap="round"
          strokeWidth={2}
        />
      </svg>
    );
  }

  const min = Math.min(...series);
  const max = Math.max(...series);
  const padding = Math.max(1, Math.round((max - min) * 0.18));
  const domainMin = Math.max(0, min - padding);
  const domainMax = max + padding;
  const range = domainMax - domainMin || 1;
  const points = series.map((value, index) => ({
    x: padX + (index / (series.length - 1)) * (w - padX * 2),
    y: padY + (h - padY * 2) - ((value - domainMin) / range) * (h - padY * 2),
  }));
  const linePath = buildSmoothPath(points);
  const areaPath = `${linePath} L${points[points.length - 1].x},${h} L${points[0].x},${h} Z`;
  const last = points[points.length - 1];

  return (
    <svg
      aria-hidden="true"
      className="compact-trend-svg"
      preserveAspectRatio="none"
      viewBox={`0 0 ${w} ${h}`}>
      <defs>
        <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.22} />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      <path d={areaPath} fill={`url(#${gradientId})`} />
      <path
        d={linePath}
        fill="none"
        stroke={color}
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
      />
      <circle cx={last.x} cy={last.y} fill={color} r={3} />
    </svg>
  );
}

function TrendDeltaBadge({ children, tone }: { children: string; tone: CompactTrendTone }) {
  return <span className={`compact-trend-delta ${getToneClass(tone)}`}>{children}</span>;
}

function buildSmoothPath(points: Array<{ x: number; y: number }>): string {
  if (points.length === 2) return `M${points[0].x},${points[0].y} L${points[1].x},${points[1].y}`;

  let path = `M${points[0].x},${points[0].y}`;
  for (let index = 0; index < points.length - 1; index += 1) {
    const p0 = points[Math.max(index - 1, 0)];
    const p1 = points[index];
    const p2 = points[index + 1];
    const p3 = points[Math.min(index + 2, points.length - 1)];
    const tension = 0.22;
    const cp1x = p1.x + (p2.x - p0.x) * tension;
    const cp1y = p1.y + (p2.y - p0.y) * tension;
    const cp2x = p2.x - (p3.x - p1.x) * tension;
    const cp2y = p2.y - (p3.y - p1.y) * tension;
    path += ` C${cp1x},${cp1y} ${cp2x},${cp2y} ${p2.x},${p2.y}`;
  }
  return path;
}

function getTrendColor(tone: CompactTrendTone): string {
  if (tone === "improving") return "var(--score-excellent)";
  if (tone === "worsening") return "var(--severity-critical)";
  if (tone === "stable") return "var(--brand)";
  return "var(--muted-foreground)";
}

function getToneClass(tone: CompactTrendTone): string {
  if (tone === "improving") return "compact-trend-delta--improving";
  if (tone === "worsening") return "compact-trend-delta--worsening";
  if (tone === "stable") return "compact-trend-delta--stable";
  return "compact-trend-delta--muted";
}
