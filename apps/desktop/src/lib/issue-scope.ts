import type { CodeIssue, CheckResult, IssueScope } from "@/lib/types";
import { formatUrlHost } from "@/lib/utils";
import { coerceJsonRecord } from "@/lib/json-record";

export interface IssueScopeMeta {
  scope: IssueScope;
  scopeLabel: "Page" | "Sitewide" | "Code";
  issueLabel: "Page issue" | "Sitewide issue" | "Code issue";
  subjectLabel: string | null;
}

interface LaunchScopeCandidate {
  id: string;
  label: string;
  description: string;
  details?: string | null;
  fixHint?: string | null;
  fixPrompt?: string | null;
  source?: string | null;
}

const PAGE_PATTERNS = [
  /(^|[._-])accessibility([._-]|$)/,
  /(^|[._-])perf([._-]|$)/,
  /alt/,
  /heading/,
  /h1/,
  /meta/,
  /title/,
  /canonical/,
  /structured[_-]?data/,
  /schema/,
  /viewport/,
  /contrast/,
  /label/,
  /tap[_-]?target/,
  /link[_-]?text/,
  /mixed[_-]?content/,
  /insecure[_-]?form/,
  /og[_-]?tags?/,
  /twitter[_-]?card/,
  /content/,
];

const SITE_PATTERNS = [
  /robots/,
  /sitemap/,
  /tls/,
  /ssl/,
  /https/,
  /hsts/,
  /csp/,
  /cors/,
  /header/,
  /cookie/,
  /redirect/,
  /compression/,
  /cache/,
  /cdn/,
  /dns/,
  /www[_-]?redirect/,
  /favicon/,
  /security[_-]?txt/,
  /analytics/,
  /bing/,
  /search[_-]?console/,
  /server/,
  /directory/,
  /exposed/,
  /env/,
  /healthcheck/,
  /error[_-]?page/,
  /404/,
];

const CODE_PATTERNS = [
  /guardrail/,
  /linked project folder/,
  /source file/,
  /code file/,
  /open in editor/,
  /reveal file/,
  /route\.ts/,
  /server action/,
  /middleware\.ts/,
  /package\.json/,
  /lockfile/,
  /src\//,
  /app\//,
  /pages\//,
];

function matchesAny(text: string, patterns: RegExp[]): boolean {
  return patterns.some((pattern) => pattern.test(text));
}

function getScopeLabels(scope: IssueScope): IssueScopeMeta {
  if (scope === "page") {
    return {
      scope,
      scopeLabel: "Page",
      issueLabel: "Page issue",
      subjectLabel: null,
    };
  }

  if (scope === "code") {
    return {
      scope,
      scopeLabel: "Code",
      issueLabel: "Code issue",
      subjectLabel: null,
    };
  }

  return {
    scope: "site",
    scopeLabel: "Sitewide",
    issueLabel: "Sitewide issue",
    subjectLabel: null,
  };
}

function getRawString(
  rawData: Record<string, unknown> | null | undefined,
  keys: string[],
): string | null {
  if (!rawData) return null;

  for (const key of keys) {
    const value = rawData[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }

  return null;
}

function formatPageSubject(value: string | null, fallbackUrl?: string | null): string | null {
  const candidate = value || fallbackUrl;
  if (!candidate) return null;

  try {
    const parsed = new URL(candidate);
    return `${parsed.pathname || "/"}${parsed.search || ""}` || "/";
  } catch {
    if (candidate.startsWith("/")) return candidate;
    return null;
  }
}

function formatSiteSubject(value: string | null, fallbackUrl?: string | null): string | null {
  const candidate = value || fallbackUrl;
  if (!candidate) return null;
  return formatUrlHost(candidate) || null;
}

function inferCheckScope(issue: CheckResult): IssueScope {
  const id = issue.checkId.toLowerCase();

  if (matchesAny(id, PAGE_PATTERNS)) return "page";
  if (matchesAny(id, SITE_PATTERNS)) return "site";

  switch (issue.category) {
    case "accessibility":
    case "performance":
      return "page";
    case "security":
    case "compliance":
    case "config":
      return "site";
    case "seo":
    case "polish":
      return "page";
    default:
      return "site";
  }
}

function inferLaunchScope(item: LaunchScopeCandidate): IssueScope {
  const combined = [
    item.id,
    item.label,
    item.description,
    item.details,
    item.fixHint,
    item.fixPrompt,
    item.source,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();

  if (matchesAny(combined, CODE_PATTERNS)) return "code";
  if (matchesAny(combined, SITE_PATTERNS)) return "site";
  if (matchesAny(combined, PAGE_PATTERNS)) return "page";

  if (item.id.startsWith("accessibility.") || item.id.startsWith("perf.")) return "page";
  if (
    item.id.startsWith("sec.") ||
    item.id.startsWith("infra.") ||
    item.id.startsWith("analytics.")
  )
    return "site";
  if (item.id.startsWith("seo."))
    return matchesAny(item.id.toLowerCase(), SITE_PATTERNS) ? "site" : "page";

  return "site";
}

export function getCheckIssueScope(issue: CheckResult, scanUrl?: string | null): IssueScopeMeta {
  const scope = inferCheckScope(issue);
  const base = getScopeLabels(scope);
  const pageValue = getRawString(coerceJsonRecord(issue.rawData), [
    "page_url",
    "final_url",
    "url",
    "page",
    "path",
    "pathname",
    "canonical_url",
  ]);
  const siteValue = getRawString(coerceJsonRecord(issue.rawData), [
    "site_url",
    "origin",
    "base_url",
    "url",
    "final_url",
  ]);

  return {
    ...base,
    subjectLabel:
      scope === "page"
        ? formatPageSubject(pageValue, scanUrl)
        : formatSiteSubject(siteValue, scanUrl),
  };
}

export function getLaunchItemScope(
  item: LaunchScopeCandidate,
  scanUrl?: string | null,
): IssueScopeMeta {
  const scope = inferLaunchScope(item);
  const base = getScopeLabels(scope);

  if (scope === "code") {
    const fileHint = [item.details, item.fixHint, item.fixPrompt]
      .filter(Boolean)
      .join("\n")
      .match(/([A-Za-z0-9._-]+\/[A-Za-z0-9_./-]+\.[A-Za-z0-9]+)/);

    return {
      ...base,
      subjectLabel: fileHint?.[1] ?? null,
    };
  }

  return {
    ...base,
    subjectLabel:
      scope === "page" ? formatPageSubject(null, scanUrl) : formatSiteSubject(null, scanUrl),
  };
}

export function getGuardrailIssueScope(issue: CodeIssue): IssueScopeMeta {
  return {
    ...getScopeLabels("code"),
    subjectLabel: issue.line ? `${issue.relativePath}:${issue.line}` : issue.relativePath,
  };
}
