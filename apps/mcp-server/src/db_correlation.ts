/** Read-only correlation queries and projections. */

import { getDb } from "./db_connection.js";
import { computeImpactScore, getFixLocationsForCheckId } from "./db_manifests.js";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export type V3Confidence = "high" | "medium" | "low";

interface TransitiveCause {
  checkId: string;
  path: string[];
  confidence: V3Confidence;
  depth: number;
}

export interface RecentEventRef {
  eventId: number;
  eventType: string;
  timestamp: string;
  title: string;
  correlationConfidence: V3Confidence;
}

type Enrichment =
  | { kind: "FieldLcp"; p75Ms: number; url: string; source: string }
  | { kind: "FieldCls"; value: number; url: string; source: string }
  | { kind: "FieldInp"; p75Ms: number; url: string; source: string }
  | { kind: "SearchImpressionsDrop"; from: number; to: number; days: number; source: string }
  | { kind: "RecentCrawlErrors"; count: number; days: number; source: string }
  | { kind: "RecentDowntime"; windowStart: string; windowEnd: string; source: string }
  | { kind: "CertExpiresIn"; days: number; source: string }
  | { kind: "CertChain"; issues: string[]; source: string }
  | { kind: "TtfbHistory"; p75Ms: number; days: number; source: string }
  | { kind: "BotTrafficPct"; value: number; source: string }
  | { kind: "CacheHitRate"; value: number; source: string }
  | { kind: "RecentFiveXxSpike"; rate: number; startedAt: string; source: string }
  | { kind: "RecentOriginErrors"; count: number; days: number; source: string }
  | { kind: "TopFallingPage"; url: string; pctDrop: number; source: string }
  | { kind: "TopFallingFunnel"; name: string; pctDrop: number; source: string };

interface CrossEnvSignal {
  stagingObservedAt: string;
  daysBeforeProd: number;
}

interface CrossProjectPattern {
  projectCount: number;
  lastSeenAt: string;
}

export interface V3IssueGroup {
  checkId: string;
  category: string;
  severity: string;
  title: string;
  description: string;
  sources: string[];
  status: string;
  impactScore: number;
  transitiveCauses: TransitiveCause[];
  downstreamEffects: string[];
  recentEvents: RecentEventRef[];
  enrichments: Enrichment[];
  affectedPages: string[];
  crossEnvSignal: CrossEnvSignal | null;
  crossProjectPattern: CrossProjectPattern | null;
  displayConfidence: V3Confidence | null;
  observationCount: number;
  anomalyScore: number | null;
}

interface CausalMapNode {
  checkId: string;
  severity: string;
  title: string;
  anomaly: boolean;
}

interface CausalMapEdge {
  from: string;
  to: string;
  confidence: V3Confidence;
}

export interface CausalMapPayload {
  nodes: CausalMapNode[];
  edges: CausalMapEdge[];
}

export interface DeployRiskPreview {
  directRisks: RiskItem[];
  downstreamRisks: RiskItem[];
  historicalRegressions: HistoricalRegression[];
}

interface RiskItem {
  checkId: string;
  severity: string;
  title: string;
  matchedFiles: string[];
  confidence: V3Confidence;
}

interface HistoricalRegression {
  checkId: string;
  deployTimestamp: string;
  scoreDrop: number;
}

export interface WhatIfResult {
  alsoResolves: WhatIfEffect[];
  confidenceBasis: Evidence[];
}

interface WhatIfEffect {
  checkId: string;
  confidence: V3Confidence;
  via: string[];
}

interface Evidence {
  kind: string;
  timestamp: string | null;
  source: string;
  detail: string;
}

interface WorkItemRow {
  check_id: string;
  category: string;
  severity: string;
  title: string;
  description: string;
  source: string;
  page_url: string | null;
}

interface CausalLinkObsRow {
  cause_check_id: string;
  effect_check_id: string;
  resolved: number;
  active: number;
}

function confidenceAsNumber(c: V3Confidence): number {
  return c === "high" ? 1.0 : c === "medium" ? 0.7 : 0.3;
}

function dynamicConfidence(base: V3Confidence, resolved: number, active: number): V3Confidence {
  if (active < 5) return base;
  const ratio = resolved / active;
  const baseVal = confidenceAsNumber(base);
  const adjusted = ratio < 0.2 ? baseVal - 0.4 : ratio > 0.7 ? baseVal + 0.2 : baseVal;
  const clamped = Math.min(1.0, Math.max(0.2, adjusted));
  if (clamped >= 0.9) return "high";
  if (clamped >= 0.5) return "medium";
  return "low";
}

/** Return the active check ids used to filter the causal graph. */
export function getActiveCheckIds(projectId: number): Set<string> {
  const db = getDb();
  const rows = db
    .prepare(
      "SELECT DISTINCT check_id FROM work_items WHERE project_id = ? AND resolved_at IS NULL",
    )
    .all(projectId) as { check_id: string }[];
  return new Set(rows.map((r) => r.check_id));
}

/** Returns recent project events tied to the supplied check ids. */
export function getRecentEvents(projectId: number, days: number): RecentEventRef[] {
  const db = getDb();
  const sinceMs = Date.now() - days * 24 * 60 * 60 * 1000;
  const rows = db
    .prepare(
      `
        SELECT e.id, e.event_type, e.occurred_at_ms, e.title, j.check_id
        FROM events e
        LEFT JOIN site_event_check_ids j ON j.event_id = e.id
        WHERE e.project_id = ? AND e.occurred_at_ms >= ?
        ORDER BY e.occurred_at_ms DESC
        LIMIT 200
      `,
    )
    .all(projectId, sinceMs) as {
    id: number;
    event_type: string;
    occurred_at_ms: number;
    title: string;
    check_id: string | null;
  }[];
  return rows.map((r) => ({
    eventId: r.id,
    eventType: r.event_type,
    timestamp: new Date(r.occurred_at_ms).toISOString(),
    title: r.title,
    correlationConfidence: "medium" as const,
  }));
}

/** Return active issue groups enriched with stored and causal-graph evidence. */
export function getActiveIssueGroupsEnriched(
  projectId: number,
  causalLinks: readonly { cause: string; effect: string; confidence: V3Confidence }[],
): V3IssueGroup[] {
  const db = getDb();

  const rows = db
    .prepare(
      `SELECT check_id, category, severity, title, description, source, page_url
       FROM work_items
       WHERE project_id = ? AND resolved_at IS NULL
       ORDER BY check_id`,
    )
    .all(projectId) as unknown as WorkItemRow[];

  if (rows.length === 0) return [];

  const grouped = new Map<string, WorkItemRow[]>();
  for (const row of rows) {
    const existing = grouped.get(row.check_id);
    if (existing) {
      existing.push(row);
    } else {
      grouped.set(row.check_id, [row]);
    }
  }

  const activeIds = new Set(grouped.keys());

  const obsMap = new Map<string, { resolved: number; active: number }>();
  const obsRows = db
    .prepare(
      `SELECT cause_check_id, effect_check_id,
                COALESCE(SUM(co_resolved), 0) AS resolved,
                COALESCE(SUM(co_active),   0) AS active
         FROM causal_link_observations
         WHERE project_id = ?
         GROUP BY cause_check_id, effect_check_id`,
    )
    .all(projectId) as unknown as CausalLinkObsRow[];
  for (const r of obsRows) {
    obsMap.set(`${r.cause_check_id}|${r.effect_check_id}`, {
      resolved: r.resolved,
      active: r.active,
    });
  }

  const recentEventsByCheckId = new Map<string, RecentEventRef[]>();
  const sinceMs = Date.now() - 30 * 24 * 60 * 60 * 1000;
  const evtRows = db
    .prepare(
      `SELECT e.id, e.event_type, e.occurred_at_ms, e.title, j.check_id
         FROM events e
         JOIN site_event_check_ids j ON j.event_id = e.id
         WHERE e.project_id = ? AND e.occurred_at_ms >= ?
         ORDER BY e.occurred_at_ms DESC`,
    )
    .all(projectId, sinceMs) as {
    id: number;
    event_type: string;
    occurred_at_ms: number;
    title: string;
    check_id: string;
  }[];
  for (const r of evtRows) {
    const ref: RecentEventRef = {
      eventId: r.id,
      eventType: r.event_type,
      timestamp: new Date(r.occurred_at_ms).toISOString(),
      title: r.title,
      correlationConfidence: "medium",
    };
    const existing = recentEventsByCheckId.get(r.check_id);
    if (existing) {
      existing.push(ref);
    } else {
      recentEventsByCheckId.set(r.check_id, [ref]);
    }
  }

  const anomalyByCheckId = new Map<string, number>();
  const anomalyRows = db
    .prepare(
      `SELECT j.check_id, e.metadata
         FROM events e
         JOIN site_event_check_ids j ON j.event_id = e.id
         WHERE e.project_id = ? AND e.event_type = 'anomaly'
         ORDER BY e.occurred_at_ms DESC`,
    )
    .all(projectId) as { check_id: string; metadata: string | null }[];
  for (const r of anomalyRows) {
    if (anomalyByCheckId.has(r.check_id)) continue;
    if (!r.metadata) continue;
    let meta: unknown;
    try {
      meta = JSON.parse(r.metadata);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      throw new Error(`Invalid anomaly metadata for '${r.check_id}': ${detail}`, { cause: error });
    }
    if (!isRecord(meta) || typeof meta.z !== "number" || !Number.isFinite(meta.z)) {
      throw new Error(`Invalid anomaly metadata z-score for '${r.check_id}'`);
    }
    anomalyByCheckId.set(r.check_id, meta.z);
  }

  // Integration hints need a generated Rust-parity manifest before MCP can
  // map cache signal keys to check ids safely.
  const enrichmentsByCheckId = new Map<string, Enrichment[]>();

  const crossEnvByCheckId = new Map<string, CrossEnvSignal>();
  const crossEnvRows = db
    .prepare(
      `SELECT wi.check_id,
                MIN(CASE WHEN env.environment != 'production'
                         THEN wi.first_seen_at END) AS staging_first_seen_ms,
                MIN(CASE WHEN env.environment = 'production' AND wi.resolved_at IS NULL
                         THEN wi.first_seen_at END) AS prod_first_seen_ms
         FROM work_items wi
         JOIN environments env ON env.project_id = wi.project_id
                               AND env.url = wi.env_url
         WHERE wi.project_id = ?
         GROUP BY wi.check_id
         HAVING staging_first_seen_ms < prod_first_seen_ms`,
    )
    .all(projectId) as {
    check_id: string;
    staging_first_seen_ms: number;
    prod_first_seen_ms: number;
  }[];
  for (const r of crossEnvRows) {
    const stagingDate = new Date(r.staging_first_seen_ms).toISOString();
    const daysBefore = Math.round(
      (r.prod_first_seen_ms - r.staging_first_seen_ms) / (24 * 60 * 60 * 1000),
    );
    crossEnvByCheckId.set(r.check_id, {
      stagingObservedAt: stagingDate,
      daysBeforeProd: daysBefore,
    });
  }

  const crossProjectByCheckId = new Map<string, CrossProjectPattern>();
  const since90dMs = Date.now() - 90 * 24 * 60 * 60 * 1000;
  const cpRows = db
    .prepare(
      `SELECT check_id,
                COUNT(DISTINCT project_id) AS project_count,
                MAX(last_seen_at) AS last_seen_ms
         FROM work_items
         WHERE check_id IN (
           SELECT DISTINCT check_id FROM work_items WHERE project_id = ? AND resolved_at IS NULL
         )
           AND project_id != ?
           AND resolved_at IS NULL
           AND last_seen_at >= ?
         GROUP BY check_id`,
    )
    .all(projectId, projectId, since90dMs) as {
    check_id: string;
    project_count: number;
    last_seen_ms: number;
  }[];
  for (const r of cpRows) {
    crossProjectByCheckId.set(r.check_id, {
      projectCount: r.project_count,
      lastSeenAt: new Date(r.last_seen_ms).toISOString(),
    });
  }

  const obsCountByCheckId = new Map<string, number>();
  for (const [key, obs] of obsMap) {
    const [causeId, effectId] = key.split("|");
    if (causeId === undefined || effectId === undefined) continue;
    for (const id of [causeId, effectId]) {
      obsCountByCheckId.set(id, (obsCountByCheckId.get(id) ?? 0) + obs.resolved);
    }
  }

  // Mirror Rust by taking the maximum calibrated confidence across active causes.
  function computeDisplayConfidence(checkId: string): V3Confidence | null {
    const likelyCauses = causalLinks
      .filter((l) => l.effect === checkId && activeIds.has(l.cause))
      .map((l) => ({ checkId: l.cause, confidence: l.confidence as V3Confidence }));

    if (likelyCauses.length === 0) return null;

    let max: V3Confidence | null = null;
    for (const cause of likelyCauses) {
      const obsKey = `${cause.checkId}|${checkId}`;
      const obs = obsMap.get(obsKey) ?? { resolved: 0, active: 0 };
      const calibrated = dynamicConfidence(cause.confidence, obs.resolved, obs.active);
      if (max === null || confidenceAsNumber(calibrated) > confidenceAsNumber(max)) {
        max = calibrated;
      }
    }
    return max;
  }

  function computeTransitiveCauses(checkId: string): TransitiveCause[] {
    const results: TransitiveCause[] = [];
    const visited = new Set<string>();
    const queue: Array<[string, string[], V3Confidence, number]> = [[checkId, [], "high", 0]];
    while (queue.length > 0) {
      const entry = queue.shift();
      if (!entry) continue;
      const [nodeId, path, minConf, depth] = entry;
      for (const link of causalLinks) {
        if (link.effect !== nodeId) continue;
        if (!activeIds.has(link.cause)) continue;
        if (visited.has(link.cause)) continue;
        visited.add(link.cause);
        const newPath = [...path, link.cause];
        const obs = obsMap.get(`${link.cause}|${nodeId}`);
        const calibrated = dynamicConfidence(
          link.confidence as V3Confidence,
          obs?.resolved ?? 0,
          obs?.active ?? 0,
        );
        const combinedConf =
          confidenceAsNumber(calibrated) < confidenceAsNumber(minConf) ? calibrated : minConf;
        results.push({
          checkId: link.cause,
          path: newPath,
          confidence: combinedConf,
          depth: depth + 1,
        });
        if (depth < 3) {
          queue.push([link.cause, newPath, combinedConf, depth + 1]);
        }
      }
    }
    return results;
  }

  function computeDownstreamEffects(checkId: string): string[] {
    const results: string[] = [];
    const visited = new Set<string>();
    const queue: string[] = [checkId];
    while (queue.length > 0) {
      const nodeId = queue.shift();
      if (!nodeId) continue;
      for (const link of causalLinks) {
        if (link.cause !== nodeId) continue;
        if (!activeIds.has(link.effect)) continue;
        if (visited.has(link.effect)) continue;
        visited.add(link.effect);
        results.push(link.effect);
        queue.push(link.effect);
      }
    }
    return results;
  }

  const groups: V3IssueGroup[] = [];
  for (const [checkId, items] of grouped) {
    const representative = items[0]!;
    const sources = [...new Set(items.map((i) => i.source))];
    const affectedPages = [
      ...new Set(items.map((i) => i.page_url).filter((p): p is string => p !== null)),
    ];
    groups.push({
      checkId,
      category: representative.category,
      severity: representative.severity,
      title: representative.title,
      description: representative.description,
      sources,
      status: "fail",
      // Same formula the desktop applies in db/work_item_groups.rs, driven by
      // the generated impact_score.json weight tables.
      impactScore: computeImpactScore(
        representative.severity,
        representative.category,
        sources.length,
      ),
      transitiveCauses: computeTransitiveCauses(checkId),
      downstreamEffects: computeDownstreamEffects(checkId),
      recentEvents: recentEventsByCheckId.get(checkId) ?? [],
      enrichments: enrichmentsByCheckId.get(checkId) ?? [],
      affectedPages,
      crossEnvSignal: crossEnvByCheckId.get(checkId) ?? null,
      crossProjectPattern: crossProjectByCheckId.get(checkId) ?? null,
      displayConfidence: computeDisplayConfidence(checkId),
      observationCount: obsCountByCheckId.get(checkId) ?? 0,
      anomalyScore: anomalyByCheckId.get(checkId) ?? null,
    });
  }

  return groups;
}

/** Return the project graph restricted to active nodes and edges. */
export function getCausalMapPayload(
  projectId: number,
  causalLinks: readonly { cause: string; effect: string; confidence: V3Confidence }[],
): CausalMapPayload {
  const db = getDb();
  const activeIds = getActiveCheckIds(projectId);

  // Fetch severity + title for each active check_id (use most severe row per check_id).
  const metaRows = db
    .prepare(
      `SELECT check_id, severity, title
       FROM work_items
       WHERE project_id = ? AND resolved_at IS NULL
       GROUP BY check_id`,
    )
    .all(projectId) as { check_id: string; severity: string; title: string }[];

  const metaByCheckId = new Map<string, { severity: string; title: string }>();
  for (const r of metaRows) {
    metaByCheckId.set(r.check_id, { severity: r.severity, title: r.title });
  }

  // Check which check_ids have recent anomaly events.
  const anomalyIds = new Set<string>();
  const anomalyRows = db
    .prepare(
      `SELECT DISTINCT j.check_id
         FROM events e
         JOIN site_event_check_ids j ON j.event_id = e.id
         WHERE e.project_id = ? AND e.event_type = 'anomaly'`,
    )
    .all(projectId) as { check_id: string }[];
  for (const r of anomalyRows) anomalyIds.add(r.check_id);

  const nodes: CausalMapNode[] = Array.from(activeIds).map((id) => {
    const meta = metaByCheckId.get(id);
    return {
      checkId: id,
      severity: meta?.severity ?? "medium",
      title: meta?.title ?? id,
      anomaly: anomalyIds.has(id),
    };
  });

  const edges: CausalMapEdge[] = causalLinks
    .filter((l) => activeIds.has(l.cause) && activeIds.has(l.effect))
    .map((l) => ({ from: l.cause, to: l.effect, confidence: l.confidence as V3Confidence }));

  return { nodes, edges };
}

/** Older app versions don't have the regressions tables yet (migration 036);
 *  that version skew is the only error allowed to degrade to []. */
function isMissingRegressionTablesError(e: unknown): boolean {
  return (
    e instanceof Error &&
    (e.message.includes("no such table: regressions") ||
      e.message.includes("no such table: regression_check_ids"))
  );
}

/** Lists stored deploy regressions for the supplied check ids. */
export function listHistoricalRegressionsForCheckIds(
  projectId: number,
  checkIds: string[],
): HistoricalRegression[] {
  if (checkIds.length === 0) return [];
  try {
    const db = getDb();
    const placeholders = checkIds.map(() => "?").join(", ");
    const rows = db
      .prepare(
        `SELECT rc.check_id AS checkId,
                r.created_at AS createdAt,
                (r.prev_score - r.score) AS scoreDrop
         FROM regression_check_ids rc
         JOIN regressions r ON r.id = rc.regression_id
         WHERE r.project_id = ? AND rc.check_id IN (${placeholders})
         ORDER BY r.created_at DESC
         LIMIT 50`,
      )
      .all(projectId, ...checkIds) as { checkId: string; createdAt: number; scoreDrop: number }[];
    return rows.map((row) => ({
      checkId: row.checkId,
      deployTimestamp: new Date(row.createdAt).toISOString(),
      scoreDrop: row.scoreDrop,
    }));
  } catch (e) {
    // Older app DB without migration 036 - degrade to the pre-feature shape.
    // Everything else (I/O errors, busy locks, corruption) must surface.
    if (isMissingRegressionTablesError(e)) return [];
    throw e;
  }
}

/** Preview direct and downstream deploy risk for changed files. */
export function previewDeployRisk(
  projectId: number,
  changedFiles: string[],
  causalLinks: readonly { cause: string; effect: string; confidence: V3Confidence }[],
): DeployRiskPreview {
  const groups = getActiveIssueGroupsEnriched(projectId, causalLinks);
  const directRisks: RiskItem[] = [];
  const downstreamCheckIds = new Set<string>();

  for (const group of groups) {
    const candidates = getFixLocationsForCheckId(group.checkId);
    const matchedPaths = new Set<string>();
    for (const candidate of candidates) {
      for (const path of candidate.paths) {
        for (const changed of changedFiles) {
          if (changed === path || changed.endsWith(`/${path}`) || changed.endsWith(path)) {
            matchedPaths.add(path);
          }
        }
      }
    }
    if (matchedPaths.size > 0) {
      directRisks.push({
        checkId: group.checkId,
        severity: group.severity,
        title: group.title,
        matchedFiles: Array.from(matchedPaths),
        confidence: "high",
      });
      for (const eff of group.downstreamEffects) {
        downstreamCheckIds.add(eff);
      }
    }
  }

  const directCheckIds = new Set(directRisks.map((r) => r.checkId));
  const downstreamRisks: RiskItem[] = groups
    .filter((g) => downstreamCheckIds.has(g.checkId) && !directCheckIds.has(g.checkId))
    .map((g) => ({
      checkId: g.checkId,
      severity: g.severity,
      title: g.title,
      matchedFiles: [],
      confidence: "medium" as V3Confidence,
    }));

  const riskCheckIds = [...directRisks, ...downstreamRisks].map((r) => r.checkId);

  return {
    directRisks,
    downstreamRisks,
    historicalRegressions: listHistoricalRegressionsForCheckIds(projectId, riskCheckIds),
  };
}

/** Predicts downstream resolutions using observed causal links. */
export function whatifResolve(
  projectId: number,
  hypotheticalResolved: string[],
  causalLinks: readonly { cause: string; effect: string; confidence: V3Confidence }[],
): WhatIfResult {
  const db = getDb();
  const activeIds = getActiveCheckIds(projectId);

  // Read causal_link_observations for calibration.
  const obsMap = new Map<string, { resolved: number; active: number }>();
  const obsRows = db
    .prepare(
      `SELECT cause_check_id, effect_check_id,
                COALESCE(SUM(co_resolved), 0) AS resolved,
                COALESCE(SUM(co_active),   0) AS active
         FROM causal_link_observations
         WHERE project_id = ?
         GROUP BY cause_check_id, effect_check_id`,
    )
    .all(projectId) as unknown as CausalLinkObsRow[];
  for (const r of obsRows) {
    obsMap.set(`${r.cause_check_id}|${r.effect_check_id}`, {
      resolved: r.resolved,
      active: r.active,
    });
  }

  // Compute also-resolves: for each hypothetically resolved id, walk forward links.
  const effects = new Map<string, { confidence: V3Confidence; via: string[] }>();
  for (const resolved of hypotheticalResolved) {
    for (const link of causalLinks) {
      if (link.cause !== resolved) continue;
      if (!activeIds.has(link.effect)) continue;
      const obs = obsMap.get(`${resolved}|${link.effect}`);
      const calibrated = dynamicConfidence(
        link.confidence as V3Confidence,
        obs?.resolved ?? 0,
        obs?.active ?? 0,
      );
      const cur = effects.get(link.effect);
      if (!cur || confidenceAsNumber(calibrated) > confidenceAsNumber(cur.confidence)) {
        effects.set(link.effect, { confidence: calibrated, via: [...(cur?.via ?? []), resolved] });
      } else {
        cur.via.push(resolved);
      }
    }
  }

  const alsoResolves: WhatIfEffect[] = Array.from(effects.entries()).map(([checkId, v]) => ({
    checkId,
    confidence: v.confidence,
    via: v.via,
  }));

  return { alsoResolves, confidenceBasis: [] };
}
