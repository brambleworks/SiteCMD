import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

import {
  getActiveIssueGroupsEnriched,
  getRecentEvents,
  getCausalMapPayload,
  previewDeployRisk,
  whatifResolve,
} from "./db.js";
import { CAUSAL_LINKS } from "./causal_graph.js";
import { READ_ONLY, runTool, text } from "./tool_result.js";
import { untrustedJson, untrustedScanData, UNTRUSTED_DATA_INSTRUCTION } from "./untrusted.js";

/** Every correlation tool accepts a project_id or a url; the caller resolves it via resolveProject. */
type ResolveProjectId = (args: { project_id?: number; url?: string }) => number;

export function registerCorrelationTools(
  server: McpServer,
  resolveProject: ResolveProjectId,
): void {
  server.registerTool(
    "get_active_correlations",
    {
      title: "Active issue correlations",
      description:
        "Returns all active issue groups for a project with v3 correlation enrichments: " +
        "transitive causes, downstream effects, recent events, integration enrichments, " +
        "cross-environment signals, cross-project patterns, observation counts, and anomaly scores. " +
        "Data is read directly from the local SiteCMD SQLite DB.",
      inputSchema: {
        project_id: z.number().int().positive().optional().describe("Project id from get_projects"),
        url: z.string().optional().describe("Site URL, used when project_id is unknown"),
      },
      annotations: READ_ONLY,
    },
    async ({ project_id, url }) =>
      runTool(() => {
        const projectId = resolveProject({ project_id, url });
        const groups = getActiveIssueGroupsEnriched(projectId, CAUSAL_LINKS);
        return text(
          [UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(untrustedJson(groups, 60000))].join(
            "\n\n",
          ),
        );
      }),
  );

  server.registerTool(
    "get_recent_events",
    {
      title: "Recent site events",
      description:
        "Returns SiteEvents tied to check_ids within the last N days. " +
        "Reads from the events and site_event_check_ids tables.",
      inputSchema: {
        project_id: z.number().int().positive().optional().describe("Project id from get_projects"),
        url: z.string().optional().describe("Site URL, used when project_id is unknown"),
        days: z.number().int().min(1).max(365).default(30),
      },
      annotations: READ_ONLY,
    },
    async ({ project_id, url, days }) =>
      runTool(() => {
        const projectId = resolveProject({ project_id, url });
        const events = getRecentEvents(projectId, days);
        return text(
          [UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(untrustedJson(events, 60000))].join(
            "\n\n",
          ),
        );
      }),
  );

  server.registerTool(
    "get_likely_causes",
    {
      title: "Likely causes of a check",
      description:
        "Returns direct and transitive likely causes for a specific check_id in a project, " +
        "computed from the active issue graph and curated causal links. " +
        "Direct causes are transitive causes at depth 1.",
      inputSchema: {
        project_id: z.number().int().positive().optional().describe("Project id from get_projects"),
        url: z.string().optional().describe("Site URL, used when project_id is unknown"),
        check_id: z.string().min(1),
      },
      annotations: READ_ONLY,
    },
    async ({ project_id, url, check_id }) =>
      runTool(() => {
        const projectId = resolveProject({ project_id, url });
        const groups = getActiveIssueGroupsEnriched(projectId, CAUSAL_LINKS);
        const target = groups.find((g) => g.checkId === check_id);
        const allCauses = target?.transitiveCauses ?? [];
        const payload = {
          checkId: check_id,
          title: target?.title ?? null,
          direct: allCauses.filter((c) => c.depth === 1),
          transitive: allCauses,
        };
        return text(
          [UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(untrustedJson(payload, 60000))].join(
            "\n\n",
          ),
        );
      }),
  );

  server.registerTool(
    "get_causal_graph",
    {
      title: "Causal graph",
      description:
        "Returns the active causal graph for a project as a node-link payload, " +
        "suitable for visualization. Nodes are active check_ids; edges connect " +
        "causally related pairs where both endpoints are currently active.",
      inputSchema: {
        project_id: z.number().int().positive().optional().describe("Project id from get_projects"),
        url: z.string().optional().describe("Site URL, used when project_id is unknown"),
      },
      annotations: READ_ONLY,
    },
    async ({ project_id, url }) =>
      runTool(() => {
        const projectId = resolveProject({ project_id, url });
        const payload = getCausalMapPayload(projectId, CAUSAL_LINKS);
        return text(
          [UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(untrustedJson(payload, 60000))].join(
            "\n\n",
          ),
        );
      }),
  );

  server.registerTool(
    "preview_deploy_risk",
    {
      title: "Preview deploy risk",
      description:
        "Given a list of files about to be changed in a deploy, predicts which active issues " +
        "are likely to regress (direct match against fix_locations.json candidate paths) " +
        "and which downstream effects might cascade via the causal graph.",
      inputSchema: {
        project_id: z.number().int().positive().optional().describe("Project id from get_projects"),
        url: z.string().optional().describe("Site URL, used when project_id is unknown"),
        changed_files: z.array(z.string()).min(1),
      },
      annotations: READ_ONLY,
    },
    async ({ project_id, url, changed_files }) =>
      runTool(() => {
        const projectId = resolveProject({ project_id, url });
        const preview = previewDeployRisk(projectId, changed_files, CAUSAL_LINKS);
        return text(
          [UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(untrustedJson(preview, 60000))].join(
            "\n\n",
          ),
        );
      }),
  );

  server.registerTool(
    "whatif_resolve",
    {
      title: "What-if resolve",
      description:
        "Given a hypothetical set of resolved check_ids, returns the downstream effects " +
        "likely to also resolve, calibrated by observation history from causal_link_observations.",
      inputSchema: {
        project_id: z.number().int().positive().optional().describe("Project id from get_projects"),
        url: z.string().optional().describe("Site URL, used when project_id is unknown"),
        hypothetical_resolved: z.array(z.string()).min(1),
      },
      annotations: READ_ONLY,
    },
    async ({ project_id, url, hypothetical_resolved }) =>
      runTool(() => {
        const projectId = resolveProject({ project_id, url });
        const result = whatifResolve(projectId, hypothetical_resolved, CAUSAL_LINKS);
        const groups = getActiveIssueGroupsEnriched(projectId, CAUSAL_LINKS);
        const titleByCheckId = new Map(groups.map((g) => [g.checkId, g.title]));
        const payload = {
          hypotheticalResolved: hypothetical_resolved.map((checkId) => ({
            checkId,
            title: titleByCheckId.get(checkId) ?? null,
          })),
          ...result,
        };
        return text(
          [UNTRUSTED_DATA_INSTRUCTION, untrustedScanData(untrustedJson(payload, 60000))].join(
            "\n\n",
          ),
        );
      }),
  );
}
