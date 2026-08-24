import React, { useState, type ReactNode } from "react";
import { ProgressBar } from "@/components/ui/progress-bar";
import { formatNum } from "@/lib/tokens";

export function MetricCard({
  detail,
  label,
  tone = "info",
  value,
}: {
  detail?: string;
  icon?: ReactNode;
  label: string;
  tone?: "info" | "success" | "warning" | "critical";
  value: string;
}) {
  const toneClass =
    tone === "success"
      ? "text-score-excellent"
      : tone === "warning"
        ? "text-severity-medium"
        : tone === "critical"
          ? "text-severity-critical"
          : "text-primary";

  return (
    <div className="tile">
      <div className="tile__rule">
        <span className="tile__label">{label}</span>
      </div>
      <p className={`metric-card__value ${toneClass}`}>{value}</p>
      {detail ? <p className="metric-card__detail">{detail}</p> : null}
    </div>
  );
}

export function BreakdownCard({
  title,
  items,
  icon,
}: {
  title: string;
  items: { label: string; value: number }[];
  icon?: ReactNode;
}) {
  const max = Math.max(...items.map((i) => i.value), 1);
  const total = items.reduce((sum, item) => sum + item.value, 0);

  return (
    <div className="card card--spacious">
      <div className="card__title-rule">
        <span className="card__title">
          {icon ? <span className="card__icon">{icon}</span> : null}
          <span>{title}</span>
        </span>
      </div>
      <div className="analytics-breakdown-list">
        {items.slice(0, 8).map((item) => {
          const pct = total > 0 ? ((item.value / total) * 100).toFixed(0) : "0";
          return (
            <div key={item.label}>
              <div className="analytics-breakdown-row text-body-muted">
                <span className="text-foreground flex-fill text-truncate">{item.label}</span>
                <span className="tabular-nums text-muted-foreground">
                  {formatNum(item.value)} <span className="text-meta">({pct}%)</span>
                </span>
              </div>
              <ProgressBar value={(item.value / max) * 100} tone="primary" label={item.label} />
            </div>
          );
        })}
      </div>
    </div>
  );
}

export const TrendChart = React.memo(function TrendChart({
  points,
  unit = "visitors",
}: {
  points: { date: string; value: number }[];
  unit?: string;
}) {
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  const values = points.map((p) => p.value);
  const max = Math.max(...values, 1);
  const W = 700;
  const H = 200;
  const padY = 20;
  const padX = 40;
  const chartW = W - padX;
  const chartH = H - padY;

  const pts = values.map((v, i) => ({
    x: padX + (i / Math.max(values.length - 1, 1)) * chartW,
    y: padY / 2 + chartH - (v / max) * chartH,
    v,
  }));

  const activePoint = activeIndex === null ? null : pts[activeIndex];
  const activeData = activeIndex === null ? null : points[activeIndex];
  const activeDate = activeData
    ? new Date(`${activeData.date}T00:00:00`).toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
      })
    : null;

  const linePath = pts
    .map((p, i) => `${i === 0 ? "M" : "L"} ${p.x.toFixed(1)} ${p.y.toFixed(1)}`)
    .join(" ");
  const areaPath =
    linePath + ` L ${pts[pts.length - 1].x.toFixed(1)} ${H} L ${pts[0].x.toFixed(1)} ${H} Z`;

  const yLabels = [0, Math.round(max / 2), max];
  const step = Math.max(1, Math.floor(points.length / 5));
  const xLabels = points
    .filter((_, i) => i % step === 0 || i === points.length - 1)
    .map((p) => {
      const idx = points.indexOf(p);
      const d = new Date(p.date);
      return {
        x: padX + (idx / Math.max(points.length - 1, 1)) * chartW,
        label: d.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
      };
    });

  return (
    <svg
      viewBox={`0 0 ${W} ${H + 16}`}
      className="trend-chart"
      preserveAspectRatio="xMidYMid meet"
      onMouseLeave={() => setActiveIndex(null)}
      onBlur={() => setActiveIndex(null)}>
      {yLabels.map((v) => {
        const y = padY / 2 + chartH - (v / max) * chartH;
        return (
          <g key={v}>
            <line x1={padX} x2={W} y1={y} y2={y} stroke="white" strokeOpacity={0.05} />
            <text
              x={padX - 6}
              y={y + 3}
              textAnchor="end"
              fill="var(--muted-foreground)"
              fontSize={9}>
              {formatNum(v)}
            </text>
          </g>
        );
      })}
      <path d={areaPath} fill="var(--primary)" fillOpacity={0.06} />
      <path d={linePath} fill="none" stroke="var(--primary)" strokeWidth={1.5} />
      {pts.map((p, i) => (
        <g key={i}>
          <circle cx={p.x} cy={p.y} r={2.5} fill="var(--primary)" opacity={0.7} />
          <circle
            cx={p.x}
            cy={p.y}
            r={10}
            fill="transparent"
            tabIndex={0}
            aria-label={`${points[i].date}: ${formatNum(p.v)} ${unit}`}
            onMouseEnter={() => setActiveIndex(i)}
            onFocus={() => setActiveIndex(i)}
          />
        </g>
      ))}
      {activePoint && activeData && activeDate ? (
        <g pointerEvents="none">
          <line
            x1={activePoint.x}
            x2={activePoint.x}
            y1={padY / 2}
            y2={H}
            stroke="var(--primary)"
            strokeOpacity={0.35}
            strokeDasharray="3 4"
          />
          <circle
            cx={activePoint.x}
            cy={activePoint.y}
            r={4}
            fill="var(--primary)"
            stroke="var(--card)"
            strokeWidth={2}
          />
          <g
            transform={`translate(${Math.min(
              Math.max(activePoint.x - 72, padX),
              W - 154,
            )}, ${Math.max(activePoint.y - 58, 8)})`}>
            <rect
              width="146"
              height="44"
              rx="8"
              fill="var(--card)"
              stroke="var(--border)"
              strokeOpacity={0.85}
            />
            <text x="12" y="17" fill="var(--muted-foreground)" fontSize={10} fontWeight={700}>
              {activeDate}
            </text>
            <text x="12" y="34" fill="var(--foreground)" fontSize={13} fontWeight={800}>
              {formatNum(activeData.value)} {unit}
            </text>
          </g>
        </g>
      ) : null}
      {xLabels.map((label, i) => (
        <text
          key={i}
          x={label.x}
          y={H + 12}
          textAnchor="middle"
          fill="var(--muted-foreground)"
          fontSize={9}>
          {label.label}
        </text>
      ))}
    </svg>
  );
});
