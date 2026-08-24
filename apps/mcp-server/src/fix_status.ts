/** Port of background/fix_attempt_watcher.rs:37-43 and core/localhost.rs:12-18. */

const LOCAL_HOSTS = new Set(["localhost", "127.0.0.1", "0.0.0.0", "::1", "[::1]"]);

function isLocalEnvironmentUrl(url: string): boolean {
  try {
    const host = new URL(url).hostname.toLowerCase();
    return LOCAL_HOSTS.has(host) || host.endsWith(".local") || host.endsWith(".localhost");
  } catch {
    return false;
  }
}

export interface FixStatusInput {
  status: string;
  check_id: string;
  producer_rule: string | null;
  env_url: string;
}

/** Remote web attempts wait for a deploy: SiteCMD rechecks every 10 minutes for 24 hours. */
export function deriveFixStatus(row: FixStatusInput): { label: string; awaitingDeploy: boolean } {
  const remoteWeb =
    row.producer_rule === null &&
    !row.check_id.startsWith("code_scan.") &&
    !isLocalEnvironmentUrl(row.env_url);
  const awaitingDeploy = row.status === "verifying" && remoteWeb;
  return { label: awaitingDeploy ? "verifying (awaiting_deploy)" : row.status, awaitingDeploy };
}

export const DEPLOY_WAIT_NOTE =
  "SiteCMD rechecks the live site every 10 minutes for up to 24 hours; if you changed source files, the fix is not live until you deploy.";
