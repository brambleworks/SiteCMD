/** Builds the SiteCMD MCP server; index.ts owns the process and the transport. */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import {
  getProjects,
  getLatestScan,
  getLiveScore,
  getIssuesForProject,
  getFixPromptsForProject,
  getIssueOccurrences,
  getIssueComparisonForProject,
  getCodeScanHistoryForProject,
  getScanHistory,
  getScanById,
  getDismissedIssues,
  getDismissedCheckIds,
  getRepoSuppressedIssues,
  getProjectByUrl,
  getFixBrief,
  requestVerification,
  listFixAttempts,
  getFixAttempt,
  getLatestFixAttemptForIssue,
  isSiteCmdDatabaseNotFoundError,
  sanitizeHistoryLimit,
  createAgentRequest,
  getAgentRequest,
  waitForAgentRequest,
  withBusyRetry,
  type Issue,
  type IssueOccurrence,
  type FixPromptRow,
} from "./db.js";
import { formatCausalityBlock, rankWithCausalReach } from "./causal_graph.js";
import { registerCorrelationTools } from "./correlation_tools.js";
import { READ_ONLY, WRITES_LOCAL_DB, runTool, text, type ToolResult } from "./tool_result.js";
import { assertSupportedSchemaVersion } from "./schema_version.js";
import {
  quoteUntrustedText,
  indentUntrustedEvidence,
  untrustedScanData,
  UNTRUSTED_DATA_INSTRUCTION,
} from "./untrusted.js";
import { describeScanAge } from "./freshness.js";
import { readDesktopHeartbeat, desktopStatusLine } from "./heartbeat.js";
import { deriveFixStatus, DEPLOY_WAIT_NOTE } from "./fix_status.js";
import {
  getWorkspaceIssues,
  getWorkspaceProject,
  getWorkspaceProjectByUrl,
  getWorkspaceScan,
  getWorkspaceScanHistory,
} from "./workspace.js";
import { MCP_SERVER_VERSION } from "./version.js";

export function createSiteCmdServer(): McpServer {
  const server = new McpServer({
    name: "sitecmd",
    version: MCP_SERVER_VERSION,
  });
  registerCoreTools(server);
  registerCorrelationTools(server, resolveProjectId);
  return server;
}

function allowWorkspaceFallbackOnlyWhenDatabaseIsMissing(error: unknown): void {
  if (!isSiteCmdDatabaseNotFoundError(error)) throw error;
}

function getProjectsWithWorkspaceFallback() {
  let projects: ReturnType<typeof getProjects>;
  try {
    projects = getProjects();
  } catch (error) {
    allowWorkspaceFallbackOnlyWhenDatabaseIsMissing(error);
    projects = [];
  }

  const workspaceProject = getWorkspaceProject();
  if (
    workspaceProject &&
    !projects.some(
      (project) =>
        (project.path && project.path === workspaceProject.path) ||
        (project.url && workspaceProject.url && project.url === workspaceProject.url),
    )
  ) {
    projects = [...projects, workspaceProject];
  }

  return projects;
}

function safeGetLiveScore(url: string): ReturnType<typeof getLiveScore> {
  // A missing desktop DB means no live score; workspace caches do not contain one.
  try {
    return getLiveScore(url);
  } catch (error) {
    allowWorkspaceFallbackOnlyWhenDatabaseIsMissing(error);
    return null;
  }
}

function getLatestScanWithWorkspaceFallback(url: string) {
  try {
    const scan = getLatestScan(url);
    if (scan) return scan;
  } catch (error) {
    allowWorkspaceFallbackOnlyWhenDatabaseIsMissing(error);
  }
  return getWorkspaceScan(url);
}

function getIssuesWithWorkspaceFallback(
  url: string,
  opts?: { min_severity?: string; category?: string },
): { issues: Issue[]; projectId: number | null } {
  const project = getProjectByUrlWithWorkspaceFallback(url);
  if (project && project.id !== 0) {
    try {
      return { issues: getIssuesForProject(project.id, url, opts), projectId: project.id };
    } catch (error) {
      allowWorkspaceFallbackOnlyWhenDatabaseIsMissing(error);
    }
  }
  return {
    issues: getWorkspaceIssues(url, { ...opts, status: "fail" }),
    projectId: null,
  };
}

function getScanHistoryWithWorkspaceFallback(url: string, limit: number) {
  try {
    const history = getScanHistory(url, limit);
    if (history.length > 0) return history;
  } catch (error) {
    allowWorkspaceFallbackOnlyWhenDatabaseIsMissing(error);
  }
  return getWorkspaceScanHistory(url, limit);
}

function getProjectByUrlWithWorkspaceFallback(url: string) {
  try {
    const project = getProjectByUrl(url);
    if (project) return project;
  } catch (error) {
    allowWorkspaceFallbackOnlyWhenDatabaseIsMissing(error);
  }
  return getWorkspaceProjectByUrl(url);
}

/** Shared by server.ts and correlation_tools.ts (passed in) to resolve a project id from either input. */
export function resolveProjectId(args: { project_id?: number; url?: string }): number {
  if (args.project_id) return args.project_id;
  if (args.url) {
    const project = getProjectByUrlWithWorkspaceFallback(args.url);
    if (project && project.id !== 0) return project.id;
    throw new Error(
      `No SiteCMD project is linked to ${args.url}. Call get_projects for ids, or add the project in SiteCMD.`,
    );
  }
  throw new Error("Pass project_id (from get_projects) or url.");
}

const SUPPORTED_AGENT_TOOLS = ["cursor", "codex", "windsurf"] as const;

/** Infer the desktop's agent_tool token from the connected MCP client name; unmatched clients brief as claude-code. */
function agentToolFromClient(server: McpServer): string {
  const clientName = server.server.getClientVersion()?.name?.toLowerCase() ?? "";
  return SUPPORTED_AGENT_TOOLS.find((tool) => clientName.includes(tool)) ?? "claude-code";
}

/**
 * Async counterpart to runTool for start_fix and run_scan, whose body awaits
 * a real timer between polls of agent_requests. runTool's body is
 * synchronous by contract, so it cannot host that wait itself.
 */
async function runToolAsync(body: () => Promise<ToolResult>): Promise<ToolResult> {
  try {
    try {
      assertSupportedSchemaVersion();
    } catch (error) {
      if (!isSiteCmdDatabaseNotFoundError(error)) throw error;
    }
    return await body();
  } catch (error) {
    return {
      content: [
        { type: "text", text: `Error: ${error instanceof Error ? error.message : String(error)}` },
      ],
      isError: true,
    };
  }
}

/**
 * url alone can resolve to more than one project: environments are unique
 * per (project_id, url), not globally, so two projects may share a
 * production URL. Prefer whichever one actually owns the open check.
 */
function resolveProjectIdForCheck(
  args: { project_id?: number; url?: string },
  checkId: string,
): number {
  if (args.project_id) return args.project_id;
  const projectId = resolveProjectId(args);
  if (!args.url) return projectId;
  const sharingUrl = getProjects().filter((p) => p.url === args.url);
  const owner = sharingUrl.find(
    (p) => getIssueOccurrences(p.id, args.url as string, checkId).length > 0,
  );
  return owner ? owner.id : projectId;
}

const CONFIDENCE_ORDER = ["confirmed", "high", "needs_review"] as const;

function meetsConfidence(issue: Issue, minimum?: string): boolean {
  if (!minimum) return true;
  const rank = CONFIDENCE_ORDER.indexOf(
    (issue.confidence ?? "needs_review") as (typeof CONFIDENCE_ORDER)[number],
  );
  return (
    rank !== -1 && rank <= CONFIDENCE_ORDER.indexOf(minimum as (typeof CONFIDENCE_ORDER)[number])
  );
}

function issueLocation(issue: Issue): string {
  if (issue.relative_path) return `${issue.relative_path}${issue.line ? `:${issue.line}` : ""}`;
  return issue.page_url ?? "(site-wide)";
}

function issueHeading(issue: Issue, level: "##" | "###"): string {
  const id = issue.id !== undefined ? ` (#${issue.id})` : "";
  return `${level} [${issue.severity.toUpperCase()}] ${quoteUntrustedText(issue.title, 500)}${id}`;
}

function issueMeta(issue: Issue): string {
  return `**Check:** ${quoteUntrustedText(issue.check_id, 200)} | **Category:** ${issue.category} | **Confidence:** ${issue.confidence ?? "unknown"} | **Where:** ${quoteUntrustedText(issueLocation(issue), 500)}`;
}

/** detail_json is untrusted scan evidence; the caller bounds and indents it before serving it. */
function prettyEvidence(detailJson: string): string | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(detailJson);
  } catch {
    return null;
  }
  return JSON.stringify(parsed, null, 2);
}

function appendIssueComparisonSection(
  lines: string[],
  heading: string,
  comparison: ReturnType<typeof getIssueComparisonForProject>,
): void {
  lines.push(`### ${heading}`, "");

  if (comparison.fixed.length > 0) {
    lines.push(`Fixed (${comparison.fixed.length})`);
    comparison.fixed.forEach((i) =>
      lines.push(`- ${quoteUntrustedText(i.title, 500)} [${i.severity}/${i.category}]`),
    );
    lines.push("");
  }
  if (comparison.newIssues.length > 0) {
    lines.push(`New Issues (${comparison.newIssues.length})`);
    comparison.newIssues.forEach((i) =>
      lines.push(`- ${quoteUntrustedText(i.title, 500)} [${i.severity}/${i.category}]`),
    );
    lines.push("");
  }
  if (comparison.remaining.length > 0) {
    lines.push(`Still Failing (${comparison.remaining.length})`);
    comparison.remaining.forEach((i) =>
      lines.push(`- ${quoteUntrustedText(i.title, 500)} [${i.severity}/${i.category}]`),
    );
    lines.push("");
  }
  if (
    comparison.fixed.length === 0 &&
    comparison.newIssues.length === 0 &&
    comparison.remaining.length === 0
  ) {
    lines.push("No issue changes found for this scan source.", "");
  }
}

function registerCoreTools(server: McpServer): void {
  server.registerTool(
    "get_projects",
    {
      title: "List SiteCMD projects",
      description:
        "List every project SiteCMD tracks with its id, production URL, framework, and linked folder. Use the id with the correlation tools and the URL with the scan tools.",
      inputSchema: {},
      annotations: READ_ONLY,
    },
    async () =>
      runTool(() => {
        const projects = getProjectsWithWorkspaceFallback();
        if (projects.length === 0) {
          return text("No projects found. Open SiteCMD and add a project first.");
        }
        const summary = projects.map(
          (p) => `- #${p.id} ${p.url || "(no URL)"}${p.framework ? ` [${p.framework}]` : ""}`,
        );
        const body = projects
          .map(
            (p) =>
              `#${p.id}\n    name: ${quoteUntrustedText(p.name, 200)}\n    path: ${quoteUntrustedText(p.path ?? "", 500)}`,
          )
          .join("\n");
        return text(
          [
            `${projects.length} project(s):\n\n${summary.join("\n")}`,
            UNTRUSTED_DATA_INSTRUCTION,
            untrustedScanData(body),
          ].join("\n\n"),
        );
      }),
  );

  server.registerTool(
    "get_scan_score",
    {
      title: "Get the SiteCMD Score",
      description: "Get the latest scan artifact score and category breakdown for a site URL",
      inputSchema: { url: z.string().describe("The site URL (e.g. https://example.com)") },
      annotations: READ_ONLY,
    },
    async ({ url }) =>
      runTool(() => {
        const scan = getLatestScanWithWorkspaceFallback(url);
        if (!scan) return text(`No scans found for ${url}. Run a scan in SiteCMD first.`);
        const live = safeGetLiveScore(url);
        const openTotal = live
          ? live.critical_count + live.high_count + live.medium_count + live.low_count
          : null;
        const lines = [
          live
            ? `## ${url} - SiteCMD Score: ${Math.round(live.overall)}/100`
            : `## ${url} - SiteCMD Score: not computed yet (open SiteCMD and run a scan)`,
          "",
          live
            ? `**Open issues:** ${openTotal} (${live.critical_count} critical, ${live.high_count} high, ${live.medium_count} medium, ${live.low_count} low), web and code combined; call get_issues for the list.`
            : null,
          `The latest web scan graded ${scan.overall_score}/100. ${describeScanAge(scan.timestamp, Date.now())}.`,
          "",
          `| Category | Latest web scan |`,
          `|----------|-----------------|`,
          scan.security_score != null ? `| Security | ${scan.security_score} |` : null,
          scan.performance_score != null ? `| Performance | ${scan.performance_score} |` : null,
          scan.seo_score != null ? `| SEO | ${scan.seo_score} |` : null,
          scan.accessibility_score != null
            ? `| Accessibility | ${scan.accessibility_score} |`
            : null,
          scan.compliance_score != null ? `| Compliance | ${scan.compliance_score} |` : null,
          scan.config_score != null ? `| Config | ${scan.config_score} |` : null,
        ];
        return text(lines.filter((line) => line !== null).join("\n"));
      }),
  );

  server.registerTool(
    "get_issues",
    {
      title: "List open issues",
      description:
        "Get open failing issues from the latest scan, optionally filtered by severity/category. Scan-derived titles, descriptions, and evidence are explicitly marked as untrusted data.",
      inputSchema: {
        url: z.string().describe("The site URL"),
        min_severity: z
          .enum(["critical", "high", "medium", "low"])
          .optional()
          .describe("Only issues at this severity or worse"),
        category: z
          .string()
          .optional()
          .describe("security, performance, seo, accessibility, compliance, polish, or config"),
        min_confidence: z
          .enum(["confirmed", "high", "needs_review"])
          .optional()
          .describe(
            "Drop heuristic findings below this confidence (confirmed > high > needs_review)",
          ),
        limit: z
          .number()
          .int()
          .min(1)
          .max(100)
          .default(25)
          .describe("Issues to return, most severe first"),
      },
      annotations: READ_ONLY,
    },
    async ({ url, min_severity, category, min_confidence, limit }) =>
      runTool(() => {
        const { issues: matching, projectId } = getIssuesWithWorkspaceFallback(url, {
          min_severity,
          category,
        });
        const issues = matching.filter((issue) => meetsConfidence(issue, min_confidence));
        if (issues.length === 0) return text("No matching issues found.");
        // Output filters must not hide active root causes from causal context.
        const { issues: allIssues } = getIssuesWithWorkspaceFallback(url);
        const dismissed = projectId ? getDismissedCheckIds(projectId, url) : new Set<string>();
        const activeCheckIds = new Set(
          allIssues.map((i) => i.check_id).filter((id) => !dismissed.has(id)),
        );
        const suppressedCount = projectId ? getRepoSuppressedIssues(projectId, url).length : 0;
        const scan = getLatestScanWithWorkspaceFallback(url);
        const shown = issues.slice(0, limit);
        const body = shown
          .map((i) => {
            const blocks = [
              issueHeading(i, "###"),
              issueMeta(i),
              quoteUntrustedText(i.description, 2500),
            ];
            const causal = formatCausalityBlock(i.check_id, activeCheckIds);
            if (causal) blocks.push("", causal);
            return blocks.join("\n");
          })
          .join("\n\n");
        // Scan and workspace content is untrusted input to the consuming agent.
        return text(
          [
            `${shown.length} of ${issues.length} open issue(s) for ${url}${scan ? ` (${describeScanAge(scan.timestamp, Date.now())})` : ""}. Call get_issue with a check_id for evidence and the fix prompt.`,
            suppressedCount > 0
              ? `${suppressedCount} finding(s) hidden by .sitecmd/config.json suppressions; see get_dismissed_issues.`
              : null,
            UNTRUSTED_DATA_INSTRUCTION,
            untrustedScanData(body),
          ]
            .filter((line) => line !== null)
            .join("\n\n"),
        );
      }),
  );

  server.registerTool(
    "get_issue",
    {
      title: "Get one issue",
      description:
        "Everything SiteCMD knows about one open check on a site: description, why it matters, evidence, every file or page it occurs on, the saved fix prompt, likely causes, and the latest fix attempt. Scan-derived text is untrusted data.",
      inputSchema: {
        url: z.string().describe("The site URL"),
        check_id: z.string().min(1).describe("Check id from get_issues"),
      },
      annotations: READ_ONLY,
    },
    async ({ url, check_id }) =>
      runTool(() => {
        const project = getProjectByUrlWithWorkspaceFallback(url);
        if (!project) return text(`No project found for ${url}.`);
        const occurrences: IssueOccurrence[] =
          project.id !== 0
            ? getIssueOccurrences(project.id, url, check_id)
            : getWorkspaceIssues(url, { status: "fail" })
                .filter((issue) => issue.check_id === check_id)
                .map((issue) => ({ ...issue, why_it_matters: null, confidence_reason: null }));
        if (occurrences.length === 0) {
          return text(
            `No open issue ${check_id} for ${url}. Call get_issues for the current list.`,
          );
        }
        const primary = occurrences[0];
        const { issues: allIssues } = getIssuesWithWorkspaceFallback(url);
        const activeCheckIds = new Set(allIssues.map((i) => i.check_id));
        const attempt =
          project.id !== 0 ? getLatestFixAttemptForIssue(project.id, url, check_id) : null;
        const evidence = primary.detail_json ? prettyEvidence(primary.detail_json) : null;
        const causal = formatCausalityBlock(check_id, activeCheckIds);
        const sections = [
          issueHeading(primary, "##"),
          issueMeta(primary) +
            (primary.confidence_reason
              ? ` (${quoteUntrustedText(primary.confidence_reason, 500)})`
              : ""),
          occurrences.length > 1
            ? `Also at: ${quoteUntrustedText(occurrences.slice(1).map(issueLocation).join(", "), 500)}`
            : null,
          `### What is wrong\n${quoteUntrustedText(primary.description, 2500)}`,
          primary.why_it_matters
            ? `### Why it matters\n${quoteUntrustedText(primary.why_it_matters, 1500)}`
            : null,
          evidence ? `### Evidence\n${indentUntrustedEvidence(evidence, 1800)}` : null,
          primary.manual_fix
            ? `### How to fix\n${quoteUntrustedText(primary.manual_fix, 3000)}`
            : null,
          primary.fix_prompt
            ? `### Fix prompt\n${quoteUntrustedText(primary.fix_prompt, 20000)}`
            : null,
          causal ? causal : null,
          attempt
            ? `Fix attempt: #${attempt.id} [${attempt.status}]${attempt.failure_detail ? ` - ${attempt.failure_detail}` : ""}`
            : "Fix attempt: none yet; the user can start one from this issue in SiteCMD.",
        ];
        const body = sections.filter((section) => section !== null).join("\n\n");
        return text([UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(body)].join("\n\n"));
      }),
  );

  server.registerTool(
    "get_fix_prompts",
    {
      title: "Get fix prompts",
      description:
        "Get AI-ready fix prompts for failing checks. Project-derived evidence is explicitly marked as untrusted data.",
      inputSchema: {
        url: z.string().describe("The site URL"),
        min_severity: z
          .enum(["critical", "high", "medium", "low"])
          .optional()
          .describe("Filter by minimum severity"),
        category: z.string().optional().describe("Filter by category"),
        check_id: z.string().optional().describe("Return only this check's prompt"),
        limit: z.number().int().min(1).max(20).default(5).describe("Fix prompts to return"),
      },
      annotations: READ_ONLY,
    },
    async ({ url, min_severity, category, check_id, limit }) =>
      runTool(() => {
        const project = getProjectByUrlWithWorkspaceFallback(url);
        if (!project) {
          return text(`No project found for ${url}.`);
        }

        let prompts: FixPromptRow[] = [];
        if (project.id !== 0) {
          try {
            prompts = getFixPromptsForProject(project.id, url, { min_severity, category });
          } catch (error) {
            allowWorkspaceFallbackOnlyWhenDatabaseIsMissing(error);
            prompts = [];
          }
        }
        if (prompts.length === 0) {
          // Fall back to workspace cache (last-scan.json retains fix_prompt on each issue)
          const wsIssues = getWorkspaceIssues(url, {
            status: "fail",
            min_severity,
            category,
          });
          prompts = wsIssues
            .filter((i) => typeof i.fix_prompt === "string" && i.fix_prompt.length > 0)
            .map((i) => ({
              title: i.title,
              severity: i.severity,
              category: i.category,
              check_id: i.check_id,
              fix_prompt: i.fix_prompt!,
            }));
        }
        if (prompts.length === 0) {
          return text(`No fix prompts available. Run a scan in SiteCMD first.`);
        }

        // Build active set from the full unfiltered scan so causal context stays complete.
        const { issues: allIssues } = getIssuesWithWorkspaceFallback(url);
        const dismissed = project.id !== 0 ? getDismissedCheckIds(project.id) : new Set<string>();
        const activeCheckIds = new Set(
          allIssues.map((i) => i.check_id).filter((id) => !dismissed.has(id)),
        );

        const ranked = rankWithCausalReach(prompts, activeCheckIds);

        const shown = check_id
          ? ranked.filter((p) => p.check_id === check_id)
          : ranked.slice(0, limit);
        if (shown.length === 0) {
          return text(
            check_id
              ? `No fix prompt for ${check_id} on ${url}; call get_issues to see open checks.`
              : `No fix prompts available for ${url}; call get_issues to see open checks.`,
          );
        }
        const body = shown
          .map((p) => {
            const causal = formatCausalityBlock(p.check_id, activeCheckIds);
            const blocks = [
              `## ${quoteUntrustedText(p.title, 500)}`,
              `**Severity:** ${p.severity} | **Category:** ${p.category} | **Check:** ${quoteUntrustedText(p.check_id, 200)}`,
            ];
            if (causal) blocks.push("", causal);
            blocks.push("", quoteUntrustedText(p.fix_prompt, 20000), "", "---");
            return blocks.join("\n");
          })
          .join("\n\n");
        const moreHint =
          !check_id && ranked.length > shown.length
            ? "; pass check_id or raise limit (max 20) for more"
            : "";
        const header = `${shown.length} of ${ranked.length} fix prompt(s) for ${url}${moreHint}:`;
        return text([header, UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(body)].join("\n\n"));
      }),
  );

  server.registerTool(
    "get_scan_history",
    {
      title: "Web scan history",
      description: "Get scan artifact score history over time for a site URL",
      inputSchema: {
        url: z.string().describe("The site URL"),
        limit: z
          .number()
          .int()
          .min(1)
          .max(100)
          .default(10)
          .describe("Recent scans to return (1-100)"),
      },
      annotations: READ_ONLY,
    },
    async ({ url, limit }) =>
      runTool(() => {
        const safeLimit = sanitizeHistoryLimit(limit);
        const history = getScanHistoryWithWorkspaceFallback(url, safeLimit);
        if (history.length === 0) {
          return text(`No scan history for ${url}.`);
        }
        const lines = [
          `## Scan History for ${url}`,
          "",
          `| Id | Date | Web scan score | Issues | Critical | High |`,
          `|----|------|-------|--------|----------|------|`,
          ...history.map(
            (s) =>
              `| #${s.scan_id} | ${s.timestamp.split("T")[0]} | ${s.overall_score} | ${s.issues_total} | ${s.issues_critical} | ${s.issues_high} |`,
          ),
        ];
        return text(lines.join("\n"));
      }),
  );

  server.registerTool(
    "get_dismissed_issues",
    {
      title: "List dismissed and suppressed issues",
      description:
        "Get issues that have been dismissed/triaged for a site - AI should skip these when suggesting fixes",
      inputSchema: { url: z.string().describe("The site URL") },
      annotations: READ_ONLY,
    },
    async ({ url }) =>
      runTool(() => {
        const project = getProjectByUrlWithWorkspaceFallback(url);
        if (!project) {
          return text(`No project found for ${url}.`);
        }
        if (project.id === 0) {
          return text(`No dismissed issues are tracked for repo-local .sitecmd scans yet.`);
        }
        const dismissed = getDismissedIssues(project.id, url);
        const suppressed = getRepoSuppressedIssues(project.id, url);
        if (dismissed.length === 0 && suppressed.length === 0) {
          return text(`No dismissed or suppressed issues for ${url}. All issues are active.`);
        }
        const dismissedLines = dismissed.map(
          (d) =>
            `- ${quoteUntrustedText(d.title ?? d.check_id, 500)} [${d.status}] (since ${d.last_status_changed_at})`,
        );
        const suppressedLines = suppressed.map(
          ({ issue, reason }) =>
            `- ${quoteUntrustedText(issue.check_id, 200)} in ${quoteUntrustedText(issue.relative_path ?? "(unknown path)", 500)}: ${quoteUntrustedText(reason, 500)}`,
        );
        const body = [
          dismissedLines.length ? `Dismissed in SiteCMD:\n${dismissedLines.join("\n")}` : null,
          suppressedLines.length
            ? `Suppressed by .sitecmd/config.json (the same rules sitecmd audit applies):\n${suppressedLines.join("\n")}`
            : null,
        ]
          .filter((line) => line !== null)
          .join("\n\n");
        return text(
          [
            `${dismissed.length} dismissed and ${suppressed.length} suppressed issue(s) for ${url}:`,
            UNTRUSTED_DATA_INSTRUCTION,
            untrustedScanData(body),
            "Skip all of these when suggesting fixes.",
          ].join("\n\n"),
        );
      }),
  );

  server.registerTool(
    "compare_scans",
    {
      title: "Compare two web scans",
      description:
        "Compare two web scans for a URL by id (default: the two most recent) - shows what was fixed, what's new, and what regressed. Use after making fixes to verify they worked.",
      inputSchema: {
        url: z.string().describe("The site URL"),
        from_scan_id: z
          .number()
          .int()
          .positive()
          .optional()
          .describe("Older scan id from get_scan_history; default: the previous scan"),
        to_scan_id: z
          .number()
          .int()
          .positive()
          .optional()
          .describe("Newer scan id; default: the latest scan"),
      },
      annotations: READ_ONLY,
    },
    async ({ url, from_scan_id, to_scan_id }) =>
      runTool(() => {
        const history = getScanHistoryWithWorkspaceFallback(url, 100);
        const latest = to_scan_id ? getScanById(url, to_scan_id) : history[0];
        const previous = from_scan_id ? getScanById(url, from_scan_id) : history[1];
        if (!latest || !previous) {
          return text(
            history.length === 0
              ? `No scans found for ${url}. Run a scan in SiteCMD first.`
              : `Could not find both scans for ${url}. Ids come from get_scan_history; the default compares the two most recent.`,
          );
        }
        const project = getProjectByUrlWithWorkspaceFallback(url);

        const scoreDelta = latest.overall_score - previous.overall_score;
        const header = [
          `## Scan Comparison for ${url}`,
          "",
          `**Scans:** #${previous.scan_id} (${previous.timestamp.split("T")[0]}) to #${latest.scan_id} (${latest.timestamp.split("T")[0]})`,
          `**Web scan score:** ${previous.overall_score}/100 to ${latest.overall_score}/100 (${scoreDelta > 0 ? "+" : ""}${scoreDelta} pts)`,
        ];

        if (!project || project.id === 0) {
          header.push(
            "",
            "Issue-level comparison is not available from repo-local scan cache yet. Run SiteCMD desktop scans for this project to compare fixed, new, and still-failing issues.",
          );
          return text(header.join("\n"));
        }

        const webComparison = getIssueComparisonForProject(
          project.id,
          url,
          previous.timestamp,
          latest.timestamp,
          "web_scan",
        );
        const issueComparisons = [webComparison];
        const body: string[] = [];
        appendIssueComparisonSection(body, "Web Scan Issues", webComparison);

        const codeHistory = getCodeScanHistoryForProject(project.id, url, 2);
        if (codeHistory.length >= 2) {
          const [latestCode, previousCode] = codeHistory;
          const codeComparison = getIssueComparisonForProject(
            project.id,
            url,
            previousCode.timestamp,
            latestCode.timestamp,
            "code_scan",
          );
          issueComparisons.push(codeComparison);
          appendIssueComparisonSection(body, "Code Scan Issues", codeComparison);
        } else {
          body.push("### Code Scan Issues", "");
          body.push("Code Scan comparison needs two Code Scans for this project.", "");
        }

        if (
          issueComparisons.some((comparison) => comparison.fixed.length > 0) &&
          issueComparisons.every(
            (comparison) => comparison.newIssues.length === 0 && comparison.remaining.length === 0,
          )
        ) {
          body.push("🎉 All issues fixed!");
        }

        return text(
          [header.join("\n"), UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(body.join("\n"))].join(
            "\n\n",
          ),
        );
      }),
  );

  const HOW_TO_RESCAN_DESCRIPTION =
    "Explain how to get fresh scan results for a site. This tool does not queue a scan; it gives the exact CLI or desktop steps and what to call afterwards."; // allow-machine-smell: negation, not the banned affirmative AI-tell

  function howToRescan(url: string): ToolResult {
    const latest = getLatestScanWithWorkspaceFallback(url);
    return text(
      [
        `## How to rescan ${url}`,
        "",
        "SiteCMD does not queue a scan from this tool. Pick one path:", // allow-machine-smell: negation, describes what SiteCMD does NOT do
        `1. CLI from the project folder: run \`sitecmd scan\` (it reads .sitecmd/config.json; if that folder is missing, run \`sitecmd init ${url}\` once). Without a config, run \`sitecmd scan --url ${url}\`. The CLI exports .sitecmd/ and syncs the desktop app when it is open.`,
        "2. Desktop: open SiteCMD, select the project, and click Scan.",
        "3. Then call `compare_scans` to see what was fixed, what is new, and what still fails.",
        "",
        latest
          ? `**Last scan:** ${describeScanAge(latest.timestamp, Date.now())}; web scan graded ${latest.overall_score}/100 with ${latest.issues_total} findings.`
          : "No previous scans found for this URL.",
      ].join("\n"),
    );
  }

  server.registerTool(
    "how_to_rescan",
    {
      title: "How to rescan a site",
      description: HOW_TO_RESCAN_DESCRIPTION,
      inputSchema: { url: z.string().describe("The site URL") },
      annotations: READ_ONLY,
    },
    async ({ url }) => runTool(() => howToRescan(url)),
  );

  server.registerTool(
    "request_scan",
    {
      title: "Deprecated alias of how_to_rescan",
      description: `Deprecated: call how_to_rescan. Kept until the next major SiteCMD release so existing agent configurations keep working. ${HOW_TO_RESCAN_DESCRIPTION}`,
      inputSchema: { url: z.string().describe("The site URL") },
      annotations: READ_ONLY,
    },
    async ({ url }) => runTool(() => howToRescan(url)),
  );

  // Fix attempt tools

  server.registerTool(
    "get_fix_brief",
    {
      title: "Read a fix brief",
      description:
        "Read the full SiteCMD fix brief for a fix attempt. The brief describes one website/code issue, where to fix it in the repository, and the acceptance criteria.",
      inputSchema: { attempt_id: z.number().int().positive() },
      annotations: READ_ONLY,
    },
    async ({ attempt_id }) =>
      runTool(() => {
        const { briefMd, status } = getFixBrief(attempt_id);
        const terminalStatuses = ["verified", "verify_failed", "canceled", "expired"];
        const statusLine = terminalStatuses.includes(status)
          ? `Warning: this fix attempt is '${status}'. Do not proceed; SiteCMD will not verify this attempt. Ask the user to start a new one from the issue in SiteCMD.`
          : status !== "briefed"
            ? `(Note: this attempt is currently '${status}'.)`
            : null;
        return text(
          [
            statusLine,
            UNTRUSTED_DATA_INSTRUCTION,
            untrustedScanData(quoteUntrustedText(briefMd, 20000)),
          ]
            .filter((line) => line !== null)
            .join("\n\n"),
        );
      }),
  );

  server.registerTool(
    "start_fix",
    {
      title: "Start a fix attempt",
      description:
        "Ask SiteCMD to open a fix attempt for one check. The desktop app writes the brief with its own guidance and file mapping; this tool waits up to 15 seconds for it, then returns the attempt id to pass to get_fix_brief. Requires the SiteCMD app to be running.",
      inputSchema: {
        project_id: z.number().int().positive().optional(),
        url: z.string().optional().describe("Site URL when project_id is unknown"),
        check_id: z.string().min(1).describe("Check id from get_issues"),
        agent_tool: z
          .enum(["claude-code", "codex", "cursor", "windsurf"])
          .optional()
          .describe("Which agent is working; defaults to the MCP client name"),
        wait: z.boolean().default(true),
      },
      annotations: WRITES_LOCAL_DB,
    },
    async ({ project_id, url, check_id, agent_tool, wait }) =>
      runToolAsync(async () => {
        const setup = withBusyRetry(() => {
          const projectId = resolveProjectIdForCheck({ project_id, url }, check_id);
          const envUrl = url ?? getProjects().find((p) => p.id === projectId)?.url;
          if (!envUrl) throw new Error(`Project #${projectId} has no production URL; pass url.`);
          if (getIssueOccurrences(projectId, envUrl, check_id).length === 0) {
            throw new Error(
              `No open issue ${check_id} on ${envUrl}; call get_issues for the current list.`,
            );
          }
          const requestId = createAgentRequest({
            kind: "start_fix",
            projectId,
            envUrl,
            checkId: check_id,
            agentTool: agent_tool ?? agentToolFromClient(server),
          });
          return { requestId, now: Date.now() };
        });
        if (!readDesktopHeartbeat(setup.now).alive || !wait) {
          return text(
            `Fix request #${setup.requestId} for ${check_id} is pending. ${desktopStatusLine(setup.now)} Call get_fix_status with request_id=${setup.requestId} to pick up the attempt id.`,
          );
        }
        const settled = await waitForAgentRequest(setup.requestId, 15_000);
        if (!settled || settled.status === "requested" || settled.status === "running") {
          return text(
            `Fix request #${setup.requestId} is still pending after 15 seconds. Call get_fix_status with request_id=${setup.requestId}.`,
          );
        }
        if (settled.status !== "fulfilled") {
          throw new Error(
            `SiteCMD could not start the fix: ${settled.failure_detail ?? settled.status}`,
          );
        }
        const { attempt_id } = JSON.parse(settled.result_json ?? "{}") as { attempt_id: number };
        return text(
          `Fix attempt #${attempt_id} is briefed. Call get_fix_brief with attempt_id=${attempt_id}, make the fix, then call request_verification with the same id and a one-paragraph summary.`,
        );
      }),
  );

  server.registerTool(
    "get_fix_status",
    {
      title: "Get fix attempt status",
      description:
        "Read the status of a fix attempt, or of a start_fix request that has not resolved to an attempt id yet. Pass attempt_id (from start_fix or get_fix_brief) or request_id (from start_fix's pending response).",
      inputSchema: {
        attempt_id: z.number().int().positive().optional(),
        request_id: z.number().int().positive().optional(),
      },
      annotations: READ_ONLY,
    },
    async ({ attempt_id, request_id }) =>
      runTool(() => {
        let resolvedAttemptId = attempt_id ?? null;
        if (resolvedAttemptId === null) {
          if (!request_id) throw new Error("Pass attempt_id or request_id.");
          const request = getAgentRequest(request_id);
          if (!request) throw new Error(`No fix request #${request_id}.`);
          if (request.status === "fulfilled") {
            const result = JSON.parse(request.result_json ?? "{}") as { attempt_id?: number };
            resolvedAttemptId = result.attempt_id ?? null;
          }
          if (resolvedAttemptId === null) {
            const now = Date.now();
            const detail = request.failure_detail
              ? ` Failure detail: ${request.failure_detail}.`
              : "";
            return text(
              `Fix request #${request_id} is '${request.status}'.${detail} ${desktopStatusLine(now)}`,
            );
          }
        }
        const attempt = getFixAttempt(resolvedAttemptId);
        if (!attempt) throw new Error(`No fix attempt with id ${resolvedAttemptId}.`);
        const now = Date.now();
        const { label, awaitingDeploy } = deriveFixStatus(attempt);
        const lines = [
          `Fix attempt #${attempt.id} for ${attempt.check_id} on ${attempt.env_url}`,
          `Status: ${label}`,
          attempt.verify_started_at
            ? `Verification started: ${new Date(attempt.verify_started_at).toISOString()}`
            : null,
          attempt.brief_fetched_at
            ? `Brief fetched: ${new Date(attempt.brief_fetched_at).toISOString()}`
            : null,
          attempt.failure_detail ? `Failure detail: ${attempt.failure_detail}` : null,
          awaitingDeploy ? DEPLOY_WAIT_NOTE : null,
          desktopStatusLine(now),
        ];
        return text(lines.filter((line) => line !== null).join("\n"));
      }),
  );

  server.registerTool(
    "run_scan",
    {
      title: "Run a SiteCMD scan",
      description:
        "Ask SiteCMD to scan a project now. The desktop app performs the scan; this tool returns a request id right away by default (pass wait=true to poll up to 90 seconds). Requires the SiteCMD app to be running.",
      inputSchema: {
        project_id: z.number().int().positive().optional(),
        url: z.string().optional().describe("Site URL when project_id is unknown"),
        scope: z.enum(["web", "code", "full"]).default("web"),
        wait: z.boolean().default(false),
      },
      annotations: WRITES_LOCAL_DB,
    },
    async ({ project_id, url, scope, wait }) =>
      runToolAsync(async () => {
        const setup = withBusyRetry(() => {
          const projectId = resolveProjectId({ project_id, url });
          const envUrl = url ?? getProjects().find((p) => p.id === projectId)?.url;
          if (!envUrl) throw new Error(`Project #${projectId} has no production URL; pass url.`);
          const requestId = createAgentRequest({
            kind: "run_scan",
            projectId,
            envUrl,
            scope,
            agentTool: agentToolFromClient(server),
          });
          return { requestId, now: Date.now() };
        });
        if (!readDesktopHeartbeat(setup.now).alive || !wait) {
          return text(
            `Scan request #${setup.requestId} (${scope}) is pending. ${desktopStatusLine(setup.now)} Call get_scan_status with request_id=${setup.requestId} for the outcome.`,
          );
        }
        const settled = await waitForAgentRequest(setup.requestId, 90_000);
        if (!settled || settled.status === "requested" || settled.status === "running") {
          return text(
            `Scan request #${setup.requestId} is still running after 90 seconds. Call get_scan_status with request_id=${setup.requestId}.`,
          );
        }
        if (settled.status !== "fulfilled") {
          throw new Error(
            `SiteCMD could not run the scan: ${settled.failure_detail ?? settled.status}`,
          );
        }
        const { execution_id, status } = JSON.parse(settled.result_json ?? "{}") as {
          execution_id: number;
          status: string;
        };
        return text(
          `Scan request #${setup.requestId} complete: execution #${execution_id} (${status}). Call compare_scans to see what changed.`,
        );
      }),
  );

  server.registerTool(
    "get_scan_status",
    {
      title: "Get scan request status",
      description: "Read the status of a run_scan request by the request id run_scan returned.",
      inputSchema: { request_id: z.number().int().positive() },
      annotations: READ_ONLY,
    },
    async ({ request_id }) =>
      runTool(() => {
        const request = getAgentRequest(request_id);
        if (!request) throw new Error(`No scan request #${request_id}.`);
        const now = Date.now();
        const lines = [`Scan request #${request.id}: ${request.status}.`];
        if (request.status === "fulfilled") {
          const result = JSON.parse(request.result_json ?? "{}") as {
            execution_id?: number;
            status?: string;
          };
          lines.push(`execution #${result.execution_id} (${result.status}).`);
          lines.push("Call compare_scans to see what changed.");
        } else if (request.status === "failed") {
          lines.push(`Failure detail: ${request.failure_detail ?? "unknown"}.`);
        } else {
          lines.push(desktopStatusLine(now));
        }
        return text(lines.join(" "));
      }),
  );

  server.registerTool(
    "request_verification",
    {
      title: "Request verification of a fix",
      description:
        "Tell SiteCMD a fix attempt is complete so it can re-run the check and verify the fix. This does NOT mark the issue fixed; SiteCMD verifies independently. Requires the SiteCMD app to be running; records the request and says so when it is not.",
      inputSchema: {
        attempt_id: z.number().int().positive(),
        summary: z.string().min(1).max(2000).describe("One paragraph describing what was changed"),
      },
      annotations: WRITES_LOCAL_DB,
    },
    async ({ attempt_id, summary }) =>
      runTool(() => {
        requestVerification(attempt_id, summary);
        const attempt = getFixAttempt(attempt_id);
        const now = Date.now();
        const liveness = readDesktopHeartbeat(now).alive
          ? "SiteCMD will re-run the check within about 5 seconds; the user sees the result in the app."
          : "SiteCMD is not running; verification starts when it opens (attempts expire after 24 hours).";
        const deploy =
          attempt && deriveFixStatus({ ...attempt, status: "verifying" }).awaitingDeploy
            ? ` This is a live-site check, so the fix is not live until you deploy; ${DEPLOY_WAIT_NOTE}`
            : "";
        return text(
          `Verification requested for attempt ${attempt_id}. ${liveness}${deploy} Call get_fix_status with attempt_id=${attempt_id} to read the outcome.`,
        );
      }),
  );

  server.registerTool(
    "list_fix_attempts",
    {
      title: "List fix attempts",
      description:
        "List SiteCMD fix attempts that are currently open (briefed, verify_requested, or verifying). Pass include_settled to also see recently verified, failed, canceled, or expired attempts.",
      inputSchema: {
        include_settled: z
          .boolean()
          .default(false)
          .describe("Also list recent verified, verify_failed, canceled, and expired attempts"),
      },
      annotations: READ_ONLY,
    },
    async ({ include_settled }) =>
      runTool(() => {
        const attempts = listFixAttempts(include_settled);
        if (attempts.length === 0) {
          return text("No open fix attempts. The user starts one from an issue in SiteCMD.");
        }
        const body = attempts
          .map((a) => `• #${a.id} ${a.checkId} [${a.status}] via ${a.agentTool}`)
          .join("\n");
        return text(body);
      }),
  );
}
