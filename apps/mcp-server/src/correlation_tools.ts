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

export function registerCorrelationTools(server: McpServer): void {
  server.tool(
    "get_active_correlations",
    "Returns all active issue groups for a project with v3 correlation enrichments: " +
      "transitive causes, downstream effects, recent events, integration enrichments, " +
      "cross-environment signals, cross-project patterns, observation counts, and anomaly scores. " +
      "Data is read directly from the local SiteCMD SQLite DB.",
    { project_id: z.number().int().positive() },
    async ({ project_id }) => {
      try {
        const groups = getActiveIssueGroupsEnriched(project_id, CAUSAL_LINKS);
        return { content: [{ type: "text", text: JSON.stringify(groups, null, 2) }] };
      } catch (e) {
        return {
          content: [{ type: "text", text: `Error: ${e instanceof Error ? e.message : String(e)}` }],
          isError: true,
        };
      }
    },
  );

  server.tool(
    "get_recent_events",
    "Returns SiteEvents tied to check_ids within the last N days. " +
      "Reads from the events and site_event_check_ids tables.",
    {
      project_id: z.number().int().positive(),
      days: z.number().int().min(1).max(365).default(30),
    },
    async ({ project_id, days }) => {
      try {
        const events = getRecentEvents(project_id, days);
        return { content: [{ type: "text", text: JSON.stringify(events, null, 2) }] };
      } catch (e) {
        return {
          content: [{ type: "text", text: `Error: ${e instanceof Error ? e.message : String(e)}` }],
          isError: true,
        };
      }
    },
  );

  server.tool(
    "get_likely_causes",
    "Returns direct and transitive likely causes for a specific check_id in a project, " +
      "computed from the active issue graph and curated causal links. " +
      "Direct causes are transitive causes at depth 1.",
    {
      project_id: z.number().int().positive(),
      check_id: z.string().min(1),
    },
    async ({ project_id, check_id }) => {
      try {
        const groups = getActiveIssueGroupsEnriched(project_id, CAUSAL_LINKS);
        const target = groups.find((g) => g.checkId === check_id);
        const allCauses = target?.transitiveCauses ?? [];
        const payload = {
          direct: allCauses.filter((c) => c.depth === 1),
          transitive: allCauses,
        };
        return { content: [{ type: "text", text: JSON.stringify(payload, null, 2) }] };
      } catch (e) {
        return {
          content: [{ type: "text", text: `Error: ${e instanceof Error ? e.message : String(e)}` }],
          isError: true,
        };
      }
    },
  );

  server.tool(
    "get_causal_graph",
    "Returns the active causal graph for a project as a node-link payload, " +
      "suitable for visualization. Nodes are active check_ids; edges connect " +
      "causally related pairs where both endpoints are currently active.",
    { project_id: z.number().int().positive() },
    async ({ project_id }) => {
      try {
        const payload = getCausalMapPayload(project_id, CAUSAL_LINKS);
        return { content: [{ type: "text", text: JSON.stringify(payload, null, 2) }] };
      } catch (e) {
        return {
          content: [{ type: "text", text: `Error: ${e instanceof Error ? e.message : String(e)}` }],
          isError: true,
        };
      }
    },
  );

  server.tool(
    "preview_deploy_risk",
    "Given a list of files about to be changed in a deploy, predicts which active issues " +
      "are likely to regress (direct match against fix_locations.json candidate paths) " +
      "and which downstream effects might cascade via the causal graph.",
    {
      project_id: z.number().int().positive(),
      changed_files: z.array(z.string()).min(1),
    },
    async ({ project_id, changed_files }) => {
      try {
        const preview = previewDeployRisk(project_id, changed_files, CAUSAL_LINKS);
        return { content: [{ type: "text", text: JSON.stringify(preview, null, 2) }] };
      } catch (e) {
        return {
          content: [{ type: "text", text: `Error: ${e instanceof Error ? e.message : String(e)}` }],
          isError: true,
        };
      }
    },
  );

  server.tool(
    "whatif_resolve",
    "Given a hypothetical set of resolved check_ids, returns the downstream effects " +
      "likely to also resolve, calibrated by observation history from causal_link_observations.",
    {
      project_id: z.number().int().positive(),
      hypothetical_resolved: z.array(z.string()).min(1),
    },
    async ({ project_id, hypothetical_resolved }) => {
      try {
        const result = whatifResolve(project_id, hypothetical_resolved, CAUSAL_LINKS);
        return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
      } catch (e) {
        return {
          content: [{ type: "text", text: `Error: ${e instanceof Error ? e.message : String(e)}` }],
          isError: true,
        };
      }
    },
  );
}
