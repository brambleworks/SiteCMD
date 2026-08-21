import type { ScanMode } from "@/components/scan/ScanConfigOverlay";
import { getProjectCapabilities } from "@/lib/project-capabilities";

export type ScanAction =
  | { kind: "error"; message: string; detail?: string }
  | { kind: "web-single"; url: string; scanType: "health"; axeEnabled: boolean }
  | { kind: "web-multi"; urls: string[]; scanType: "health"; axeEnabled: boolean }
  | { kind: "code"; projectId: number; folder: string; url: string };

interface PlanScanInput {
  mode: ScanMode;
  /** URLs from the ScanConfig, or empty/undefined to fall back to activeUrl. */
  urls?: string[];
  /** The active environment's URL, or null for a code-only project. */
  activeUrl: string | null;
  activeProjectId: number | null;
  projectFolder: string | null;
  axeEnabled?: boolean;
}

const NO_CODE_ERROR: ScanAction = {
  kind: "error",
  message: "Code Scan needs a linked project folder",
  detail: "Link a local folder to this project before running this scan.",
};

const NO_SITE_ERROR: ScanAction = {
  kind: "error",
  message: "Web Scan needs a site URL",
  detail: "Add an environment URL to this project before running this scan.",
};

const NOTHING_TO_SCAN_ERROR: ScanAction = {
  kind: "error",
  message: "Nothing to scan yet",
  detail: "Add a site URL or link a local folder to this project first.",
};

function buildWebAction(urls: string[], axeEnabled: boolean): ScanAction {
  return urls.length > 1
    ? { kind: "web-multi", urls, scanType: "health", axeEnabled }
    : { kind: "web-single", url: urls[0], scanType: "health", axeEnabled };
}

export function planScan(input: PlanScanInput): ScanAction[] {
  const { mode, urls, activeUrl, activeProjectId, projectFolder, axeEnabled = false } = input;
  const { hasSite, hasCode } = getProjectCapabilities({
    environmentUrl: activeUrl,
    projectFolder,
  });
  // A code scan is addressed by project id, so an unselected project is as
  // disqualifying as a missing folder.
  const canRunCode = hasCode && activeProjectId != null && projectFolder != null;

  // Web/Full both map to backend "health". Config URLs win over the active
  // environment, but a project with no site has nothing to fall back to.
  const requestedUrls = urls && urls.length > 0 ? urls : activeUrl ? [activeUrl] : [];
  const webUrls = requestedUrls.filter((url) => url.trim());
  const canRunWeb = hasSite && webUrls.length > 0;

  if (mode === "code") {
    if (!canRunCode) return [NO_CODE_ERROR];
    return [
      { kind: "code", projectId: activeProjectId, folder: projectFolder, url: activeUrl ?? "" },
    ];
  }

  if (mode === "web") {
    if (!canRunWeb) return [NO_SITE_ERROR];
    return [buildWebAction(webUrls, axeEnabled)];
  }

  // Full: run every half this project has, in web-then-code order so the
  // sequential executor sees the live site before the codebase.
  const actions: ScanAction[] = [];
  if (canRunWeb) actions.push(buildWebAction(webUrls, axeEnabled));
  if (canRunCode) {
    actions.push({
      kind: "code",
      projectId: activeProjectId,
      folder: projectFolder,
      url: activeUrl ?? "",
    });
  }

  return actions.length > 0 ? actions : [NOTHING_TO_SCAN_ERROR];
}
