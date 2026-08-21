import { command } from "./invoke";
import type { BaselineDecisionResult, SiteBaseline } from "@/generated/ipc-bindings";

/** Read the site's verified-good baseline. */
export function getSiteBaseline(args: {
  siteId: number;
  projectId?: number;
  environmentScopeKey?: string;
}): Promise<SiteBaseline> {
  return command<SiteBaseline>("get_site_baseline", args);
}

/** Accept or dismiss a baseline change under revision and digest guards. */
export function decideSiteBaseline(args: {
  siteId: number;
  field: string;
  basedOnRevision: number;
  expectedDigest: string;
  accept: boolean;
  projectId?: number;
  environmentScopeKey?: string;
}): Promise<BaselineDecisionResult> {
  return command<BaselineDecisionResult>("decide_site_baseline", args);
}
