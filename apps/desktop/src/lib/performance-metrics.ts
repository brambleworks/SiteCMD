import { isJsonRecord } from "@/lib/json-record";

const PERFORMANCE_METRICS_STORAGE_KEY = "sitecmd_performance_metrics_v1";
const PERFORMANCE_SAMPLE_LIMIT = 12;

export const PERFORMANCE_BUDGETS = {
  "app.cold_start_ms": { label: "Cold app start", budgetMs: 2500 },
  "app.first_project_load_ms": { label: "First project load", budgetMs: 1500 },
  "scan.duration_ms": { label: "First scan duration", budgetMs: 20000 },
  "issues.initial_ready_ms": { label: "Issues page render", budgetMs: 1200 },
  "events.initial_ready_ms": { label: "Activity page load", budgetMs: 1500 },
} as const;

type PerformanceMetricKey = keyof typeof PERFORMANCE_BUDGETS;

type PerformanceMetricMeta = Record<string, string | number | boolean | null | undefined>;

interface PerformanceMetricSample {
  durationMs: number;
  recordedAt: string;
  meta: Record<string, string | number | boolean | null>;
}

interface StoredPerformanceMetrics {
  metrics: Partial<Record<PerformanceMetricKey, PerformanceMetricSample[]>>;
}

interface PerformanceMetricSummary {
  key: PerformanceMetricKey;
  label: string;
  budgetMs: number;
  count: number;
  firstDurationMs: number | null;
  latestDurationMs: number | null;
  averageDurationMs: number | null;
  lastRecordedAt: string | null;
  latestMeta: Record<string, string | number | boolean | null>;
  withinBudget: boolean | null;
}

export interface PerformanceTimer {
  key: PerformanceMetricKey;
  startedAt: number;
  meta: Record<string, string | number | boolean | null>;
}

function nowMs() {
  if (typeof performance !== "undefined" && typeof performance.now === "function") {
    return performance.now();
  }
  return Date.now();
}

function serializeMeta(meta?: PerformanceMetricMeta | null) {
  if (!meta) return {};
  return Object.fromEntries(
    Object.entries(meta)
      .map(([key, value]) => [key, normalizeMetaValue(value)] as const)
      .filter(
        (entry): entry is [string, string | number | boolean | null] => entry[1] !== undefined,
      ),
  ) as Record<string, string | number | boolean | null>;
}

function readStoredState(): StoredPerformanceMetrics {
  if (typeof window === "undefined") return { metrics: {} };
  try {
    const raw = window.localStorage.getItem(PERFORMANCE_METRICS_STORAGE_KEY);
    if (!raw) return { metrics: {} };
    return parseStoredState(JSON.parse(raw) as unknown);
  } catch {
    return { metrics: {} };
  }
}

function parseStoredState(value: unknown): StoredPerformanceMetrics {
  if (!isJsonRecord(value) || !isJsonRecord(value.metrics)) return { metrics: {} };
  const metrics: StoredPerformanceMetrics["metrics"] = {};
  for (const key of Object.keys(PERFORMANCE_BUDGETS) as PerformanceMetricKey[]) {
    const samples = value.metrics[key];
    if (!Array.isArray(samples)) continue;
    metrics[key] = samples
      .flatMap((sample) => parseMetricSample(sample) ?? [])
      .slice(-PERFORMANCE_SAMPLE_LIMIT);
  }
  return { metrics };
}

function parseMetricSample(value: unknown): PerformanceMetricSample | null {
  if (
    !isJsonRecord(value) ||
    typeof value.durationMs !== "number" ||
    !Number.isFinite(value.durationMs) ||
    value.durationMs < 0 ||
    typeof value.recordedAt !== "string"
  ) {
    return null;
  }
  return {
    durationMs: Math.round(value.durationMs),
    recordedAt: value.recordedAt,
    meta: parsePrimitiveMetaRecord(value.meta),
  };
}

function parsePrimitiveMetaRecord(
  value: unknown,
): Record<string, string | number | boolean | null> {
  if (!isJsonRecord(value)) return {};
  return Object.fromEntries(
    Object.entries(value)
      .map(([key, entry]) => [key, normalizeMetaValue(entry)] as const)
      .filter(
        (entry): entry is [string, string | number | boolean | null] => entry[1] !== undefined,
      ),
  );
}

function normalizeMetaValue(value: unknown): string | number | boolean | null | undefined {
  if (value == null) return null;
  if (typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") return Number.isFinite(value) ? value : undefined;
  return undefined;
}

function writeStoredState(state: StoredPerformanceMetrics) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(PERFORMANCE_METRICS_STORAGE_KEY, JSON.stringify(state));
  } catch {
    // best effort
  }
}

function average(values: number[]) {
  if (values.length === 0) return null;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

export function startPerformanceTimer(
  key: PerformanceMetricKey,
  meta?: PerformanceMetricMeta,
): PerformanceTimer {
  return {
    key,
    startedAt: nowMs(),
    meta: serializeMeta(meta),
  };
}

export function recordPerformanceMetric(
  key: PerformanceMetricKey,
  durationMs: number,
  meta?: PerformanceMetricMeta,
) {
  if (!Number.isFinite(durationMs) || durationMs < 0) return;
  const state = readStoredState();
  const sample: PerformanceMetricSample = {
    durationMs: Math.round(durationMs),
    recordedAt: new Date().toISOString(),
    meta: {
      ...serializeMeta(meta),
    },
  };
  const nextSamples = [...(state.metrics[key] ?? []), sample].slice(-PERFORMANCE_SAMPLE_LIMIT);
  state.metrics[key] = nextSamples;
  writeStoredState(state);
}

export function finishPerformanceTimer(
  timer: PerformanceTimer | null | undefined,
  meta?: PerformanceMetricMeta,
) {
  if (!timer) return;
  recordPerformanceMetric(timer.key, nowMs() - timer.startedAt, {
    ...timer.meta,
    ...serializeMeta(meta),
  });
}

export function finishPerformanceTimerAfterPaint(
  timer: PerformanceTimer | null | undefined,
  meta?: PerformanceMetricMeta,
) {
  if (!timer) return;
  const finalize = () => finishPerformanceTimer(timer, meta);
  if (typeof window === "undefined" || typeof window.requestAnimationFrame !== "function") {
    finalize();
    return;
  }
  window.requestAnimationFrame(() => {
    window.requestAnimationFrame(() => finalize());
  });
}

export function readPerformanceSnapshot(): PerformanceMetricSummary[] {
  const state = readStoredState();
  return (Object.keys(PERFORMANCE_BUDGETS) as PerformanceMetricKey[]).map((key) => {
    const samples = state.metrics[key] ?? [];
    const durations = samples.map((sample) => sample.durationMs);
    const latest = samples[samples.length - 1] ?? null;
    const budgetMs = PERFORMANCE_BUDGETS[key].budgetMs;
    return {
      key,
      label: PERFORMANCE_BUDGETS[key].label,
      budgetMs,
      count: samples.length,
      firstDurationMs: samples[0]?.durationMs ?? null,
      latestDurationMs: latest?.durationMs ?? null,
      averageDurationMs: average(durations),
      lastRecordedAt: latest?.recordedAt ?? null,
      latestMeta: latest?.meta ?? {},
      withinBudget: latest ? latest.durationMs <= budgetMs : null,
    };
  });
}

export function buildPerformanceSnapshotText() {
  const lines = [
    "SiteCMD Performance Snapshot",
    `Generated: ${new Date().toLocaleString()}`,
    "-".repeat(40),
  ];
  for (const metric of readPerformanceSnapshot()) {
    if (metric.count === 0) {
      lines.push(`${metric.label}: pending`);
      continue;
    }
    const averageText =
      metric.averageDurationMs == null ? "n/a" : `${Math.round(metric.averageDurationMs)}ms avg`;
    const latestText =
      metric.latestDurationMs == null ? "n/a" : `${metric.latestDurationMs}ms latest`;
    const firstText = metric.firstDurationMs == null ? "n/a" : `${metric.firstDurationMs}ms first`;
    const budgetText = `${metric.budgetMs}ms budget`;
    const statusText =
      metric.withinBudget == null
        ? "pending"
        : metric.withinBudget
          ? "within budget"
          : "over budget";
    const metaText =
      Object.keys(metric.latestMeta).length > 0
        ? ` (${Object.entries(metric.latestMeta)
            .map(([key, value]) => `${key}=${value}`)
            .join(", ")})`
        : "";
    lines.push(
      `${metric.label}: ${latestText}, ${firstText}, ${averageText}, ${budgetText}, ${statusText}${metaText}`,
    );
  }
  return lines.join("\n");
}

export function clearPerformanceSnapshot() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(PERFORMANCE_METRICS_STORAGE_KEY);
  } catch {
    // best effort
  }
}
