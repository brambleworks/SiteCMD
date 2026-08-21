import { parseJsonRecord } from "@/lib/json-record";

export function parseAlertDetailRecord(json: string | null): Record<string, unknown> {
  if (!json) return {};
  return parseJsonRecord(json) ?? {};
}

interface DeployRegressionCommit {
  hash: string;
  shortHash: string;
  message: string;
  author: string;
  date: string;
}

export interface DeployRegressionDetail {
  scanKind: "web" | "code";
  scanId: number;
  regressionId: number;
  previousScore: number;
  currentScore: number;
  /** Signed previous-minus-current score; deploy blame does not imply a drop. */
  scoreDrop: number;
  newIssues: { checkId: string; title: string }[];
  fixedCount: number;
  /** Real findings excluded from deploy attribution because their checks changed. */
  detectorChangedCount: number;
  /** The release that produced the current scan, when it was recorded. */
  engineRelease: string;
  commitFrom: string;
  commitTo: string;
  commitCount: number;
  commits: DeployRegressionCommit[];
}

export function parseDeployRegressionDetail(
  record: Record<string, unknown>,
): DeployRegressionDetail | null {
  if (record.alert_type !== "deploy_regression") return null;
  const scanKind = record.scan_kind === "code" ? "code" : "web";
  const asNumber = (value: unknown): number => (typeof value === "number" ? value : 0);
  const asString = (value: unknown): string => (typeof value === "string" ? value : "");
  const newIssues = Array.isArray(record.new_issues)
    ? record.new_issues.flatMap((entry) => {
        if (typeof entry !== "object" || entry === null) return [];
        const item = entry as Record<string, unknown>;
        const checkId = asString(item.check_id);
        if (!checkId) return [];
        return [{ checkId, title: asString(item.title) || checkId }];
      })
    : [];
  const commits = Array.isArray(record.commits)
    ? record.commits.flatMap((entry) => {
        if (typeof entry !== "object" || entry === null) return [];
        const item = entry as Record<string, unknown>;
        const hash = asString(item.hash);
        if (!hash) return [];
        return [
          {
            hash,
            shortHash: asString(item.short_hash) || hash.slice(0, 7),
            message: asString(item.message),
            author: asString(item.author),
            date: asString(item.date),
          },
        ];
      })
    : [];
  const regressionId = asNumber(record.regression_id);
  const commitTo = asString(record.commit_to);
  // Reject records that violate the Rust writer's regression identity invariants.
  if (regressionId === 0 && !commitTo) return null;
  return {
    scanKind,
    scanId: asNumber(record.scan_id),
    regressionId,
    previousScore: asNumber(record.previous_score),
    currentScore: asNumber(record.current_score),
    scoreDrop: asNumber(record.score_drop),
    newIssues,
    fixedCount: asNumber(record.fixed_count),
    detectorChangedCount: asNumber(record.detector_changed_count),
    engineRelease: asString(record.engine_release),
    commitFrom: asString(record.commit_from),
    commitTo,
    commitCount: asNumber(record.commit_count),
    commits,
  };
}
