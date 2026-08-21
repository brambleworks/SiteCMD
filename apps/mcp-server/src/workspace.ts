import { existsSync, readFileSync } from "fs";
import { dirname, join } from "path";
import { type Issue, type Project, type ScanScore } from "./db.js";
import { severityMatchesMinimum, severityRank } from "./severity.js";

/** CLI workspace issue shape persisted in `.sitecmd/last-scan.json`. */
export interface WorkspaceIssue extends Issue {
  id?: number;
  status: string;
  manual_fix: string | null;
}

interface CliConfig {
  version: number;
  url: string;
  name: string;
  environments?: Record<string, string>;
}

interface CategorySummary {
  category?: string;
  name?: string;
  score?: number;
}

interface WorkspaceScanResult {
  url: string;
  overall_score: number;
  categories?: CategorySummary[];
  issues: WorkspaceIssue[];
  timestamp: string;
  scan_type?: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function nullableString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function requiredString(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

function finiteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

const WORKSPACE_SEVERITIES = new Set(["critical", "high", "medium", "low"]);
const WORKSPACE_CHECK_STATUSES = new Set(["pass", "fail", "warn", "skipped"]);
const WORKSPACE_CATEGORIES = new Set([
  "security",
  "performance",
  "seo",
  "accessibility",
  "compliance",
  "config",
  "polish",
]);
const WORKSPACE_SCAN_TYPES = new Set(["health", "security", "accessibility", "polish"]);

function optionalStringIsValid(value: unknown): boolean {
  return value === undefined || value === null || typeof value === "string";
}

export function parseWorkspaceIssue(input: unknown): WorkspaceIssue | null {
  if (!isRecord(input)) return null;

  const category = requiredString(input.category);
  const checkId = requiredString(input.check_id);
  const severity = requiredString(input.severity);
  const status = requiredString(input.status);
  const title = requiredString(input.title);
  const description = typeof input.description === "string" ? input.description : null;

  if (
    !category ||
    !WORKSPACE_CATEGORIES.has(category) ||
    !checkId ||
    !severity ||
    !WORKSPACE_SEVERITIES.has(severity) ||
    !status ||
    !WORKSPACE_CHECK_STATUSES.has(status) ||
    !title ||
    description === null ||
    !optionalStringIsValid(input.fix_prompt) ||
    !optionalStringIsValid(input.manual_fix) ||
    !optionalStringIsValid(input.page_url) ||
    (input.id !== undefined && input.id !== null && !Number.isSafeInteger(input.id))
  ) {
    return null;
  }

  return {
    id: finiteNumber(input.id) ?? undefined,
    category,
    check_id: checkId,
    severity,
    status,
    title,
    description,
    fix_prompt: nullableString(input.fix_prompt),
    manual_fix: nullableString(input.manual_fix),
    page_url: nullableString(input.page_url),
  };
}

function parseCategorySummary(input: unknown): CategorySummary | null {
  if (!isRecord(input)) return null;

  if (
    !optionalStringIsValid(input.category) ||
    !optionalStringIsValid(input.name) ||
    (input.score !== undefined && input.score !== null && finiteNumber(input.score) === null)
  ) {
    return null;
  }

  const category = nullableString(input.category);
  const name = nullableString(input.name);
  const score = finiteNumber(input.score);
  if (category && !WORKSPACE_CATEGORIES.has(category)) return null;
  if (score !== null && (score < 0 || score > 100)) return null;
  if (!category && !name && score === null) return null;

  return {
    category: category ?? undefined,
    name: name ?? undefined,
    score: score ?? undefined,
  };
}

export function parseWorkspaceScanResult(input: unknown): WorkspaceScanResult | null {
  if (!isRecord(input)) return null;

  const url = requiredString(input.url);
  const overallScore = finiteNumber(input.overall_score);
  const timestamp = requiredString(input.timestamp);
  if (
    !url ||
    overallScore === null ||
    overallScore < 0 ||
    overallScore > 100 ||
    !timestamp ||
    !Number.isFinite(Date.parse(timestamp))
  ) {
    return null;
  }
  if (!Array.isArray(input.issues)) return null;

  if (!optionalStringIsValid(input.scan_type)) return null;
  const scanType = nullableString(input.scan_type);
  if (scanType && !WORKSPACE_SCAN_TYPES.has(scanType)) return null;

  const issues: WorkspaceIssue[] = [];
  for (const issue of input.issues) {
    const parsed = parseWorkspaceIssue(issue);
    if (!parsed) return null;
    issues.push(parsed);
  }

  let categories: CategorySummary[] | undefined;
  if (input.categories !== undefined && input.categories !== null) {
    if (!Array.isArray(input.categories)) return null;
    categories = [];
    for (const category of input.categories) {
      const parsed = parseCategorySummary(category);
      if (!parsed) return null;
      categories.push(parsed);
    }
  }

  return {
    url,
    overall_score: overallScore,
    categories,
    issues,
    timestamp,
    scan_type: scanType ?? undefined,
  };
}

function parseCliConfig(input: unknown): CliConfig | null {
  if (!isRecord(input)) return null;

  const version = finiteNumber(input.version);
  const url = requiredString(input.url);
  const name = requiredString(input.name);
  if (version === null || !url || !name) return null;

  const environments = isRecord(input.environments)
    ? Object.fromEntries(
        Object.entries(input.environments).filter(
          (entry): entry is [string, string] =>
            typeof entry[0] === "string" && typeof entry[1] === "string",
        ),
      )
    : undefined;

  return {
    version,
    url,
    name,
    environments,
  };
}

function normalizeUrl(url: string): string {
  return url.trim().replace(/\/$/, "");
}

function findSitecmdDir(start = process.cwd()): string | null {
  let current = start;
  while (true) {
    const candidate = join(current, ".sitecmd");
    if (existsSync(candidate)) {
      return candidate;
    }
    const parent = dirname(current);
    if (parent === current) return null;
    current = parent;
  }
}

export function parsePackageDependencyNames(input: unknown): Set<string> | null {
  if (!isRecord(input)) return null;

  const dependencyGroups = [input.dependencies, input.devDependencies];
  const names = new Set<string>();

  for (const dependencyGroup of dependencyGroups) {
    if (!isRecord(dependencyGroup)) continue;
    for (const dependencyName of Object.keys(dependencyGroup)) {
      if (dependencyName.trim().length > 0) names.add(dependencyName);
    }
  }

  return names;
}

function detectFramework(projectRoot: string): string | null {
  const packageJsonPath = join(projectRoot, "package.json");
  if (!existsSync(packageJsonPath)) return null;
  const deps = parsePackageDependencyNames(readJson(packageJsonPath));
  if (!deps) return null;
  if (deps.has("next")) return "Next.js";
  if (deps.has("astro")) return "Astro";
  if (deps.has("react")) return "React";
  if (deps.has("vue")) return "Vue";
  if (deps.has("svelte")) return "Svelte";
  return null;
}

function readJson(path: string): unknown | null {
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

function readWorkspaceJson(path: string, label: string): unknown | null {
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error(
      `SiteCMD ${label} at ${path} is unreadable or invalid JSON. Regenerate the workspace cache before relying on its scan results.`,
    );
  }
}

function loadWorkspaceScan(sitecmdDir: string): WorkspaceScanResult | null {
  const path = join(sitecmdDir, "last-scan.json");
  const raw = readWorkspaceJson(path, "scan cache");
  if (raw === null) return null;
  const scan = parseWorkspaceScanResult(raw);
  if (!scan) {
    throw new Error(
      `SiteCMD scan cache at ${path} has an invalid envelope, issue row, or category row. Regenerate it before relying on its findings.`,
    );
  }
  return scan;
}

function getCategoryScore(scan: WorkspaceScanResult, key: string): number | null {
  const match = scan.categories?.find((category) => {
    const raw = (category.category ?? category.name ?? "").toString().toLowerCase();
    return raw === key;
  });
  return typeof match?.score === "number" ? match.score : null;
}

function buildWorkspaceScanScore(scan: WorkspaceScanResult): ScanScore {
  const failingIssues = scan.issues.filter((issue) => issue.status !== "pass");
  return {
    scan_id: 0,
    url: scan.url,
    overall_score: scan.overall_score,
    security_score: getCategoryScore(scan, "security"),
    performance_score: getCategoryScore(scan, "performance"),
    seo_score: getCategoryScore(scan, "seo"),
    accessibility_score: getCategoryScore(scan, "accessibility"),
    compliance_score: getCategoryScore(scan, "compliance"),
    config_score: getCategoryScore(scan, "config") ?? getCategoryScore(scan, "polish"),
    issues_total: failingIssues.length,
    issues_critical: failingIssues.filter((issue) => issue.severity === "critical").length,
    issues_high: failingIssues.filter((issue) => issue.severity === "high").length,
    timestamp: scan.timestamp,
  };
}

export function getWorkspaceProject(): Project | null {
  const sitecmdDir = findSitecmdDir();
  if (!sitecmdDir) return null;
  const projectRoot = dirname(sitecmdDir);
  const configPath = join(sitecmdDir, "config.json");
  const rawConfig = readWorkspaceJson(configPath, "project config");
  if (rawConfig === null) return null;
  const config = parseCliConfig(rawConfig);
  if (!config) {
    throw new Error(
      `SiteCMD project config at ${configPath} has an invalid schema. Regenerate it before relying on workspace fallback data.`,
    );
  }
  return {
    id: 0,
    name: config.name,
    path: projectRoot,
    framework: detectFramework(projectRoot),
    url: config.url,
  };
}

export function getWorkspaceProjectByUrl(url: string): Project | null {
  const project = getWorkspaceProject();
  if (!project?.url) return null;
  return normalizeUrl(project.url) === normalizeUrl(url) ? project : null;
}

export function getWorkspaceScan(url: string): ScanScore | null {
  const sitecmdDir = findSitecmdDir();
  if (!sitecmdDir) return null;
  const scan = loadWorkspaceScan(sitecmdDir);
  if (!scan) return null;
  const project = getWorkspaceProject();
  const knownUrls = new Set<string>([
    normalizeUrl(scan.url),
    ...(project?.url ? [normalizeUrl(project.url)] : []),
  ]);
  if (normalizeUrl(url) !== "" && !knownUrls.has(normalizeUrl(url))) {
    return null;
  }
  return buildWorkspaceScanScore(scan);
}

export function getWorkspaceIssues(
  url: string,
  opts?: {
    status?: string;
    severity?: string;
    severityMode?: "exact" | "minimum";
    category?: string;
  },
): WorkspaceIssue[] {
  const sitecmdDir = findSitecmdDir();
  if (!sitecmdDir) return [];
  const scan = loadWorkspaceScan(sitecmdDir);
  if (!scan) return [];
  const score = getWorkspaceScan(url);
  if (!score) return [];
  return scan.issues
    .filter((issue) => !opts?.status || issue.status === opts.status)
    .filter((issue) => {
      if (!opts?.severity) return true;
      if (opts.severityMode === "minimum") {
        return severityMatchesMinimum(issue.severity, opts.severity);
      }
      return issue.severity === opts.severity;
    })
    .filter((issue) => !opts?.category || issue.category === opts.category)
    .sort((a, b) => {
      return severityRank(a.severity) - severityRank(b.severity) || a.title.localeCompare(b.title);
    });
}

export function getWorkspaceScanHistory(url: string, limit = 10): ScanScore[] {
  const scan = getWorkspaceScan(url);
  return scan ? [scan].slice(0, limit) : [];
}
