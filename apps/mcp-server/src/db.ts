/** Public SQLite query facade for MCP tools. */

import { getDb, isSiteCmdDatabaseNotFoundError } from "./db_connection.js";
import { OFFLINE_GRACE_PERIOD_SECS } from "./db_manifests.js";
import { severitiesAtOrAbove } from "./severity.js";
import { applyRepoSuppressions, type SuppressedView } from "./suppressions.js";

export {
  __test_isReadDbReadonly,
  __test_readBusyTimeout,
  isSiteCmdDatabaseNotFoundError,
  resolveDbPath,
  withBusyRetry,
} from "./db_connection.js";
export {
  __test_impactScoreGrid,
  computeImpactScore,
  parseFixLocationsManifest,
  parseImpactScoreManifest,
  parseLicenseConstantsManifest,
} from "./db_manifests.js";
export * from "./db_correlation.js";
export * from "./db_fix_attempts.js";

export type Tier = "free" | "core" | "pro";

export const SUPPORTED_ISSUE_STATUSES = ["fail"] as const;
type SupportedIssueStatus = (typeof SUPPORTED_ISSUE_STATUSES)[number];

function isSupportedIssueStatus(status: string): status is SupportedIssueStatus {
  return (SUPPORTED_ISSUE_STATUSES as readonly string[]).includes(status);
}

function addMinimumSeverityFilter(sql: string, params: unknown[], severity?: string): string {
  if (!severity) return sql;
  const severities = severitiesAtOrAbove(severity);
  params.push(...severities);
  return `${sql} AND severity IN (${severities.map(() => "?").join(", ")})`;
}

function parseIsoDate(value: string | null | undefined): Date | null {
  if (!value) return null;
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

/** Mirror the desktop rule that an expired snooze is active again. */
function effectiveIssueStatus(
  rawStatus: string,
  snoozeUntil: number | null,
  nowMs: number,
): string {
  if (rawStatus === "snoozed" && snoozeUntil !== null && snoozeUntil <= nowMs) {
    return "new";
  }
  return rawStatus;
}

/** Statuses the desktop's get_inactive_check_ids treats as not-active. */
const DISMISSED_STATUSES = new Set(["snoozed", "ignored", "blocked", "verified"]);

export function getEffectiveTier(): Tier {
  try {
    const db = getDb();
    const row = db
      .prepare(
        `
      SELECT tier, status, last_validated_at
      FROM license_state
      WHERE id = 1
      LIMIT 1
    `,
      )
      .get() as { tier?: string; status?: string; last_validated_at?: string } | undefined;

    if (!row || row.status !== "active") return "free";

    const lastValidated = parseIsoDate(row.last_validated_at);
    if (!lastValidated) return "free";

    const elapsedSeconds = Math.floor((Date.now() - lastValidated.getTime()) / 1000);
    if (elapsedSeconds < 0) return "free";
    if (elapsedSeconds > OFFLINE_GRACE_PERIOD_SECS) return "free";

    return row.tier === "core" || row.tier === "pro" ? row.tier : "free";
  } catch (error) {
    if (isSiteCmdDatabaseNotFoundError(error)) return "free";
    throw error;
  }
}

export function sanitizeHistoryLimit(limit: number): number {
  // One abuse bound on query size; no tier exists to key on.
  const normalized = Number.isFinite(limit) ? Math.max(1, Math.floor(limit)) : 10;
  return Math.min(normalized, 100);
}

export interface Project {
  id: number;
  name: string;
  path: string;
  framework: string | null;
  url: string | null;
}

export function getProjects(): Project[] {
  const db = getDb();
  return db
    .prepare(
      `
    SELECT p.id, p.name, p.path, p.framework,
           (SELECT e.url FROM environments e WHERE e.project_id = p.id AND e.environment = 'production' LIMIT 1) as url
    FROM projects p
    ORDER BY p.name
  `,
    )
    .all() as unknown as Project[];
}

export interface ScanScore {
  scan_id: number;
  url: string;
  overall_score: number;
  security_score: number | null;
  performance_score: number | null;
  seo_score: number | null;
  accessibility_score: number | null;
  compliance_score: number | null;
  config_score: number | null;
  issues_total: number;
  issues_critical: number;
  issues_high: number;
  timestamp: string;
}

function envUrlVariants(envUrl: string): [string, string] {
  const trimmed = envUrl.replace(/\/+$/, "");
  return [trimmed, `${trimmed}/`];
}

/** The latest deduplicated SiteCMD Score, distinct from raw scan scores. */
export interface LiveScore {
  overall: number;
  critical_count: number;
  high_count: number;
  medium_count: number;
  low_count: number;
  exploitable_capped: boolean;
  computed_at: number;
}

export function getLiveScore(url: string): LiveScore | null {
  const project = getProjectByUrl(url);
  if (!project) return null;
  const db = getDb();
  const [noSlash, withSlash] = envUrlVariants(url);
  const row = db
    .prepare(
      `
    SELECT overall, critical_count, high_count, medium_count, low_count,
           exploitable_capped, computed_at
    FROM score_snapshots
    WHERE project_id = ? AND environment_url IN (?, ?)
    ORDER BY id DESC
    LIMIT 1
  `,
    )
    .get(project.id, noSlash, withSlash) as
    | {
        overall: number;
        critical_count: number;
        high_count: number;
        medium_count: number;
        low_count: number;
        exploitable_capped: number;
        computed_at: number;
      }
    | undefined;
  if (!row) return null;
  return {
    overall: row.overall,
    critical_count: row.critical_count,
    high_count: row.high_count,
    medium_count: row.medium_count,
    low_count: row.low_count,
    exploitable_capped: row.exploitable_capped !== 0,
    computed_at: row.computed_at,
  };
}

export function getLatestScan(url: string): ScanScore | null {
  const db = getDb();
  const [noSlash, withSlash] = envUrlVariants(url);
  const row = db
    .prepare(
      `
    SELECT execution.id AS scan_id,
           COALESCE(execution.environment_url, run.environment_url) AS url,
           run.raw_score AS overall_score,
           run.security_score, run.performance_score, run.seo_score,
           run.accessibility_score, run.compliance_score, run.config_score,
           (SELECT COUNT(*)
              FROM scan_findings finding
              JOIN scan_runs evidence ON evidence.id = finding.run_id
             WHERE evidence.execution_id = execution.id
               AND evidence.source = 'web_scan'
               AND finding.verdict IN ('fail', 'warn')) AS issues_total,
           (SELECT COUNT(*)
              FROM scan_findings finding
              JOIN scan_runs evidence ON evidence.id = finding.run_id
             WHERE evidence.execution_id = execution.id
               AND evidence.source = 'web_scan'
               AND finding.verdict IN ('fail', 'warn')
               AND finding.severity = 'critical') AS issues_critical,
           (SELECT COUNT(*)
              FROM scan_findings finding
              JOIN scan_runs evidence ON evidence.id = finding.run_id
             WHERE evidence.execution_id = execution.id
               AND evidence.source = 'web_scan'
               AND finding.verdict IN ('fail', 'warn')
               AND finding.severity = 'high') AS issues_high,
           run.timestamp_text AS timestamp
    FROM scan_executions execution
    JOIN scan_runs run ON run.id = (
      SELECT candidate.id
      FROM scan_runs candidate
      WHERE candidate.execution_id = execution.id
        AND candidate.source = 'web_scan'
        AND candidate.run_kind IN ('multi_parent', 'single')
      ORDER BY CASE candidate.run_kind WHEN 'multi_parent' THEN 0 ELSE 1 END,
               candidate.id DESC
      LIMIT 1
    )
    WHERE execution.environment_scope_key IN (?, ?)
      AND execution.status IN ('complete', 'partial')
      AND run.raw_score IS NOT NULL
    ORDER BY execution.started_at DESC, execution.id DESC
    LIMIT 1
  `,
    )
    .get(noSlash, withSlash) as ScanScore | undefined;
  return row ?? null;
}

export interface Issue {
  id?: number;
  source?: string;
  category: string;
  check_id: string;
  severity: string;
  title: string;
  description: string;
  fix_prompt: string | null;
  page_url: string | null;
  relative_path?: string | null;
  line?: number | null;
  confidence?: string | null;
  detail_json?: string | null;
}

function excludeDismissedCheckIds<T extends { check_id: string }>(
  projectId: number,
  envUrl: string,
  rows: T[],
): T[] {
  const dismissed = getDismissedCheckIds(projectId, envUrl);
  if (dismissed.size === 0) return rows;
  return rows.filter((row) => !dismissed.has(row.check_id));
}

export function getProjectPathById(projectId: number): string | null {
  const row = getDb().prepare(`SELECT path FROM projects WHERE id = ?`).get(projectId) as
    { path: string } | undefined;
  return row?.path ? row.path : null;
}

function loadOpenIssueRows(
  projectId: number,
  envUrl: string,
  opts?: { min_severity?: string; category?: string; requireFixPrompt?: boolean },
): SuppressedView<Issue> {
  const db = getDb();
  const [noSlash, withSlash] = envUrlVariants(envUrl);
  let sql = `SELECT id, source, category, check_id, severity, title, description, fix_prompt, page_url,
                    relative_path, line, confidence, detail_json
             FROM work_items
             WHERE project_id = ? AND env_url IN (?, ?) AND resolved_at IS NULL
               AND source IN ('web_scan', 'code_scan')`;
  const params: unknown[] = [projectId, noSlash, withSlash];
  if (opts?.requireFixPrompt) sql += ` AND fix_prompt IS NOT NULL AND fix_prompt != ''`;
  sql = addMinimumSeverityFilter(sql, params, opts?.min_severity);
  if (opts?.category) {
    sql += ` AND category = ?`;
    params.push(opts.category);
  }
  sql += ` ORDER BY CASE severity
    WHEN 'critical' THEN 0
    WHEN 'high' THEN 1
    WHEN 'medium' THEN 2
    WHEN 'low' THEN 3
    ELSE 4 END, title, id`;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const rows = db.prepare(sql).all(...(params as any[])) as unknown as Issue[];
  return applyRepoSuppressions(
    getProjectPathById(projectId),
    excludeDismissedCheckIds(projectId, envUrl, rows),
    new Date(),
  );
}

export function getRepoSuppressedIssues(
  projectId: number,
  envUrl: string,
): Array<{ issue: Issue; reason: string }> {
  return loadOpenIssueRows(projectId, envUrl).ignored.map(({ row, reason }) => ({
    issue: row,
    reason,
  }));
}

export function getIssuesForProject(
  projectId: number,
  envUrl: string,
  opts?: {
    status?: string;
    min_severity?: string;
    category?: string;
  },
): Issue[] {
  if (opts?.status && !isSupportedIssueStatus(opts.status)) {
    return [];
  }

  return loadOpenIssueRows(projectId, envUrl, {
    min_severity: opts?.min_severity,
    category: opts?.category,
  }).kept;
}

export type IssueOccurrence = Issue & {
  manual_fix: string | null;
  why_it_matters: string | null;
  confidence_reason: string | null;
};

/** Every open occurrence of one check, most severe first; code findings may occur in many files. */
export function getIssueOccurrences(
  projectId: number,
  envUrl: string,
  checkId: string,
): IssueOccurrence[] {
  const [noSlash, withSlash] = envUrlVariants(envUrl);
  const rows = getDb()
    .prepare(
      `SELECT id, source, category, check_id, severity, title, description, fix_prompt, page_url,
              relative_path, line, confidence, detail_json, manual_fix, why_it_matters, confidence_reason
       FROM work_items
       WHERE project_id = ? AND env_url IN (?, ?) AND resolved_at IS NULL
         AND source IN ('web_scan', 'code_scan') AND check_id = ?
       ORDER BY CASE severity
         WHEN 'critical' THEN 0
         WHEN 'high' THEN 1
         WHEN 'medium' THEN 2
         WHEN 'low' THEN 3
         ELSE 4 END, relative_path, line, id`,
    )
    .all(projectId, noSlash, withSlash, checkId) as unknown as IssueOccurrence[];
  return applyRepoSuppressions(
    getProjectPathById(projectId),
    excludeDismissedCheckIds(projectId, envUrl, rows),
    new Date(),
  ).kept;
}

export interface FixPromptRow {
  title: string;
  severity: string;
  category: string;
  check_id: string;
  fix_prompt: string;
}

export function getFixPromptsForProject(
  projectId: number,
  envUrl: string,
  opts?: {
    min_severity?: string;
    category?: string;
  },
): FixPromptRow[] {
  return loadOpenIssueRows(projectId, envUrl, { ...opts, requireFixPrompt: true }).kept.map(
    ({ title, severity, category, check_id, fix_prompt }) => ({
      title,
      severity,
      category,
      check_id,
      fix_prompt: fix_prompt ?? "",
    }),
  );
}

interface IssueComparisonRow extends Issue {
  first_seen_at: number;
  resolved_at: number | null;
}

interface IssueComparison {
  fixed: Issue[];
  newIssues: Issue[];
  remaining: Issue[];
}

type IssueSource = "web_scan" | "code_scan";

export function getIssueComparisonForProject(
  projectId: number,
  envUrl: string,
  previousTimestamp: string,
  latestTimestamp: string,
  source: IssueSource = "web_scan",
): IssueComparison {
  const previousMs = Date.parse(previousTimestamp);
  const latestMs = Date.parse(latestTimestamp);
  if (!Number.isFinite(previousMs) || !Number.isFinite(latestMs)) {
    throw new Error("Cannot compare scans because scan history timestamps are invalid.");
  }

  const db = getDb();
  const [noSlash, withSlash] = envUrlVariants(envUrl);
  const rows = db
    .prepare(
      `SELECT category, check_id, severity, title, description, fix_prompt, page_url,
              first_seen_at, resolved_at
       FROM work_items
       WHERE project_id = ? AND env_url IN (?, ?)
         AND source = ?
         AND (
           (resolved_at IS NULL AND first_seen_at <= ?)
           OR (resolved_at > ? AND resolved_at <= ?)
         )
       ORDER BY CASE severity
         WHEN 'critical' THEN 0
         WHEN 'high' THEN 1
         WHEN 'medium' THEN 2
         WHEN 'low' THEN 3
         ELSE 4 END, title`,
    )
    .all(
      projectId,
      noSlash,
      withSlash,
      source,
      latestMs,
      previousMs,
      latestMs,
    ) as unknown as IssueComparisonRow[];

  const toIssue = ({
    category,
    check_id,
    severity,
    title,
    description,
    fix_prompt,
    page_url,
  }: IssueComparisonRow): Issue => ({
    category,
    check_id,
    severity,
    title,
    description,
    fix_prompt,
    page_url,
  });

  return {
    fixed: rows.filter((issue) => issue.resolved_at !== null).map(toIssue),
    newIssues: rows
      .filter(
        (issue) =>
          issue.resolved_at === null &&
          issue.first_seen_at > previousMs &&
          issue.first_seen_at <= latestMs,
      )
      .map(toIssue),
    remaining: rows
      .filter((issue) => issue.resolved_at === null && issue.first_seen_at <= previousMs)
      .map(toIssue),
  };
}

interface CodeScanHistoryRow {
  scan_id: number;
  project_id: number;
  environment_url: string | null;
  overall_score: number;
  issue_count: number;
  critical_count: number;
  high_count: number;
  timestamp: string;
}

export function getCodeScanHistoryForProject(
  projectId: number,
  envUrl: string,
  limit = 10,
): CodeScanHistoryRow[] {
  const db = getDb();
  const [noSlash, withSlash] = envUrlVariants(envUrl);
  return db
    .prepare(
      `
      SELECT run.id AS scan_id, run.project_id, run.environment_url,
             run.raw_score AS overall_score,
             (SELECT COUNT(*) FROM scan_findings finding
               WHERE finding.run_id = run.id
                 AND finding.verdict IN ('fail', 'warn')) AS issue_count,
             (SELECT COUNT(*) FROM scan_findings finding
               WHERE finding.run_id = run.id
                 AND finding.verdict IN ('fail', 'warn')
                 AND finding.severity = 'critical') AS critical_count,
             (SELECT COUNT(*) FROM scan_findings finding
               WHERE finding.run_id = run.id
                 AND finding.verdict IN ('fail', 'warn')
                 AND finding.severity = 'high') AS high_count,
             run.timestamp_text AS timestamp
      FROM scan_runs run
      WHERE run.project_id = ?
        AND run.source = 'code_scan'
        AND run.run_kind = 'code'
        AND run.status = 'complete'
        AND run.raw_score IS NOT NULL
        AND (run.environment_url IS NULL OR run.environment_scope_key IN (?, ?))
      ORDER BY run.started_at DESC, run.id DESC
      LIMIT ?
    `,
    )
    .all(projectId, noSlash, withSlash, limit) as unknown as CodeScanHistoryRow[];
}

interface DismissedIssue {
  check_id: string;
  env_url: string;
  status: string;
  title: string | null;
  last_status_changed_at: string;
}

interface IssueStateRow {
  check_id: string;
  env_url: string;
  status: string;
  snooze_until: number | null;
  last_status_changed_at: number;
  title: string | null;
}

/** Read lifecycle states from the same store used by SiteCMD Score. */
function getDismissedIssueStateRows(projectId: number, envUrl?: string): IssueStateRow[] {
  const db = getDb();
  const nowMs = Date.now();
  let sql = `SELECT s.check_id, s.env_url, s.status, s.snooze_until, s.last_status_changed_at,
              (SELECT w.title FROM work_items w
                WHERE w.project_id = s.project_id AND w.check_id = s.check_id
                ORDER BY w.last_seen_at DESC LIMIT 1) AS title
       FROM project_issue_states s
       WHERE s.project_id = ?`;
  const params: unknown[] = [projectId];
  if (envUrl) {
    const [noSlash, withSlash] = envUrlVariants(envUrl);
    sql += ` AND s.env_url IN (?, ?)`;
    params.push(noSlash, withSlash);
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const rows = db.prepare(sql).all(...(params as any[])) as unknown as IssueStateRow[];
  return rows.filter((row) =>
    DISMISSED_STATUSES.has(effectiveIssueStatus(row.status, row.snooze_until, nowMs)),
  );
}

export function getDismissedIssues(projectId: number, envUrl?: string): DismissedIssue[] {
  return getDismissedIssueStateRows(projectId, envUrl).map((row) => ({
    check_id: row.check_id,
    env_url: row.env_url,
    status: row.status,
    title: row.title,
    last_status_changed_at: new Date(row.last_status_changed_at).toISOString(),
  }));
}

/** Return currently inactive check ids, treating expired snoozes as active. */
export function getDismissedCheckIds(projectId: number, envUrl?: string): Set<string> {
  return new Set(getDismissedIssueStateRows(projectId, envUrl).map((row) => row.check_id));
}

export function getProjectByUrl(url: string): Project | null {
  const db = getDb();
  const [noSlash, withSlash] = envUrlVariants(url);
  const row = db
    .prepare(
      `
    SELECT p.id, p.name, p.path, p.framework,
           e.url
    FROM projects p
    JOIN environments e ON e.project_id = p.id
    WHERE e.url IN (?, ?)
    LIMIT 1
  `,
    )
    .get(noSlash, withSlash) as Project | undefined;
  return row ?? null;
}

export function getScanHistory(url: string, limit = 10): ScanScore[] {
  const db = getDb();
  const [noSlash, withSlash] = envUrlVariants(url);
  return db
    .prepare(
      `
    SELECT execution.id AS scan_id,
           COALESCE(execution.environment_url, run.environment_url) AS url,
           run.raw_score AS overall_score,
           run.security_score, run.performance_score, run.seo_score,
           run.accessibility_score, run.compliance_score, run.config_score,
           (SELECT COUNT(*)
              FROM scan_findings finding
              JOIN scan_runs evidence ON evidence.id = finding.run_id
             WHERE evidence.execution_id = execution.id
               AND evidence.source = 'web_scan'
               AND finding.verdict IN ('fail', 'warn')) AS issues_total,
           (SELECT COUNT(*)
              FROM scan_findings finding
              JOIN scan_runs evidence ON evidence.id = finding.run_id
             WHERE evidence.execution_id = execution.id
               AND evidence.source = 'web_scan'
               AND finding.verdict IN ('fail', 'warn')
               AND finding.severity = 'critical') AS issues_critical,
           (SELECT COUNT(*)
              FROM scan_findings finding
              JOIN scan_runs evidence ON evidence.id = finding.run_id
             WHERE evidence.execution_id = execution.id
               AND evidence.source = 'web_scan'
               AND finding.verdict IN ('fail', 'warn')
               AND finding.severity = 'high') AS issues_high,
           run.timestamp_text AS timestamp
    FROM scan_executions execution
    JOIN scan_runs run ON run.id = (
      SELECT candidate.id
      FROM scan_runs candidate
      WHERE candidate.execution_id = execution.id
        AND candidate.source = 'web_scan'
        AND candidate.run_kind IN ('multi_parent', 'single')
      ORDER BY CASE candidate.run_kind WHEN 'multi_parent' THEN 0 ELSE 1 END,
               candidate.id DESC
      LIMIT 1
    )
    WHERE execution.environment_scope_key IN (?, ?)
      AND execution.status IN ('complete', 'partial')
      AND run.raw_score IS NOT NULL
    ORDER BY execution.started_at DESC, execution.id DESC
    LIMIT ?
  `,
    )
    .all(noSlash, withSlash, limit) as unknown as ScanScore[];
}

/** Scan ids come from get_scan_history; the workspace cache has no ids (scan_id 0). */
export function getScanById(url: string, scanId: number): ScanScore | null {
  return getScanHistory(url, 100).find((scan) => scan.scan_id === scanId) ?? null;
}
