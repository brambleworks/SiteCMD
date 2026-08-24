/* eslint-disable react-refresh/only-export-components -- trend helpers are exported here. */
import type { ScanCategory, ScanResult } from "@/lib/types";
import { ProgressBar } from "@/components/ui/progress-bar";
import { CATEGORY_CSS_VAR, CATEGORY_LABELS, CATEGORY_ORDER } from "@/lib/tokens";
import { Button } from "@/components/ui/button";

export interface ScoreTrendPoint {
  overall: number;
  security: number | null;
  performance: number | null;
  seo: number | null;
  accessibility: number | null;
  compliance: number | null;
  config: number | null;
  polish?: number | null;
  timestamp: string;
  issues: number;
  scanType: string;
}

interface CategoryData {
  category: ScanCategory;
  score: number;
  issues: number;
}

export function Sparkline({
  data,
  height = 60,
  color = "var(--primary)",
  padX = 24,
  padY = 12,
  showEndpoints = true,
}: {
  data: number[];
  height?: number;
  color?: string;
  padX?: number;
  padY?: number;
  showEndpoints?: boolean;
}) {
  if (data.length < 2) return null;
  const w = 400;
  const h = height;

  const min = Math.max(0, Math.min(...data) - 8);
  const max = Math.min(100, Math.max(...data) + 8);
  const range = max - min || 1;

  const points = data.map((value, index) => ({
    x: padX + (index / (data.length - 1)) * (w - padX * 2),
    y: padY + (h - padY * 2) - ((value - min) / range) * (h - padY * 2),
  }));

  const smoothLine = (() => {
    if (points.length === 2) return `M${points[0].x},${points[0].y} L${points[1].x},${points[1].y}`;
    let path = `M${points[0].x},${points[0].y}`;
    for (let index = 0; index < points.length - 1; index++) {
      const p0 = points[Math.max(index - 1, 0)];
      const p1 = points[index];
      const p2 = points[index + 1];
      const p3 = points[Math.min(index + 2, points.length - 1)];
      const tension = 0.3;
      const cp1x = p1.x + (p2.x - p0.x) * tension;
      const cp1y = p1.y + (p2.y - p0.y) * tension;
      const cp2x = p2.x - (p3.x - p1.x) * tension;
      const cp2y = p2.y - (p3.y - p1.y) * tension;
      path += ` C${cp1x},${cp1y} ${cp2x},${cp2y} ${p2.x},${p2.y}`;
    }
    return path;
  })();

  const areaPath = `${smoothLine} L${points[points.length - 1].x},${h} L${points[0].x},${h} Z`;
  const last = points[points.length - 1];
  const first = points[0];
  const gradientId = `spark-grad-${data.length}-${data[0]}`;

  return (
    <svg
      viewBox={`0 0 ${w} ${h}`}
      className="trend-chart"
      height={height}
      preserveAspectRatio="none">
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.25} />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      <path d={areaPath} fill={`url(#${gradientId})`} />
      <path
        d={smoothLine}
        fill="none"
        stroke={color}
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {showEndpoints && (
        <>
          <circle cx={first.x} cy={first.y} r={2.5} fill={color} opacity={0.6} />
          <circle cx={last.x} cy={last.y} r={6} fill={color} fillOpacity={0.15} />
          <circle cx={last.x} cy={last.y} r={3} fill={color} />
        </>
      )}
    </svg>
  );
}

/** Category sparklines derived from trend history rather than one scan's detail. */
export function CategoryTrendGrid({
  trend,
  detail,
  onSelectCategory,
  exclude = [],
}: {
  trend: ScoreTrendPoint[];
  detail: ScanResult | null;
  onSelectCategory: (category: ScanCategory) => void;
  exclude?: ScanCategory[];
}) {
  if (trend.length === 0) return null;

  const issueCountByCategory = new Map<ScanCategory, number>();
  if (detail?.categories) {
    for (const cat of detail.categories) {
      issueCountByCategory.set(cat.category, cat.issuesTotal);
    }
  }

  const excludeSet = new Set(exclude);
  const cards = CATEGORY_ORDER.filter((category) => !excludeSet.has(category))
    .map((category) => {
      const series = extractCategorySeries(trend, category);
      if (series.length === 0) return null;
      const score = series[series.length - 1];
      const delta = series.length > 1 ? score - series[0] : null;
      return {
        category,
        score,
        series,
        delta,
        issues: issueCountByCategory.get(category) ?? 0,
      };
    })
    .filter((card): card is NonNullable<typeof card> => card !== null);

  if (cards.length === 0) return null;

  return (
    <div className="category-trend-grid">
      {cards.map((card) => (
        <CategoryTrendCard
          key={card.category}
          category={card.category}
          score={card.score}
          issues={card.issues}
          series={card.series}
          delta={card.delta}
          onClick={() => onSelectCategory(card.category)}
        />
      ))}
    </div>
  );
}

function extractCategorySeries(trend: ScoreTrendPoint[], category: ScanCategory): number[] {
  const values: number[] = [];
  for (const point of trend) {
    const raw = (() => {
      switch (category) {
        case "security":
          return point.security;
        case "performance":
          return point.performance;
        case "seo":
          return point.seo;
        case "accessibility":
          return point.accessibility;
        case "compliance":
          return point.compliance;
        case "config":
          return point.config;
        case "polish":
          return point.polish ?? null;
        default:
          return null;
      }
    })();
    if (typeof raw === "number" && raw > 0) values.push(raw);
  }
  return values;
}

function CategoryTrendCard({
  category,
  score,
  issues,
  series,
  delta,
  onClick,
}: {
  category: ScanCategory;
  score: number;
  issues: number;
  series: number[];
  delta: number | null;
  onClick: () => void;
}) {
  const label = CATEGORY_LABELS[category] ?? category;
  const cssVar = CATEGORY_CSS_VAR[category] ?? "var(--primary)";
  const hasTrend = series.length >= 2;

  return (
    <Button unstyled type="button" onClick={onClick} className="stat-card category-trend-card">
      <div className="category-trend-head">
        <span className="text-micro category-trend-label">{label}</span>
        {delta != null && delta !== 0 && (
          <span
            className={`text-micro category-trend-delta ${
              delta > 0 ? "text-score-excellent" : "text-severity-high"
            }`}>
            {delta > 0 ? "+" : ""}
            {delta}
          </span>
        )}
      </div>
      <div className="category-trend-score-row">
        <span className="metric-value category-trend-value">{score}</span>
        <span className="text-meta category-trend-pct">%</span>
        {issues > 0 && (
          <span className="text-meta tabular-nums">
            {issues} issue{issues === 1 ? "" : "s"}
          </span>
        )}
      </div>
      <div className="category-trend-spark">
        {hasTrend ? (
          <Sparkline
            data={series}
            height={32}
            color={cssVar}
            padX={4}
            padY={4}
            showEndpoints={false}
          />
        ) : (
          <ProgressBar
            percent={score}
            color={cssVar}
            label={`${label} score`}
            trackClassName="category-trend-track"
          />
        )}
      </div>
    </Button>
  );
}

export function buildCategoryScores(
  trend: ScoreTrendPoint,
  detail: ScanResult | null,
): CategoryData[] {
  if (detail?.categories) {
    return detail.categories
      .filter((category) => category.score > 0)
      .sort((a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category))
      .map((category) => ({
        category: category.category,
        score: category.score,
        issues: category.issuesTotal,
      }));
  }

  const categories = CATEGORY_ORDER.map((key) => ({ key, score: trend[key] ?? null }));

  return categories
    .filter((category) => category.score != null && category.score > 0)
    .map((category) => ({ category: category.key, score: category.score!, issues: 0 }));
}
