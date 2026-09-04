import type { ScanScore } from "./db.js";
import { describeScanAge } from "./freshness.js";
import {
  indentUntrustedEvidence,
  untrustedScanData,
  UNTRUSTED_DATA_INSTRUCTION,
} from "./untrusted.js";

const MAX_RESCAN_URL_LENGTH = 8192;

function normalizeRescanUrl(value: string): string {
  if (value.length > MAX_RESCAN_URL_LENGTH || /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/u.test(value)) {
    throw new Error(
      "The site URL must be at most 8192 characters and contain no control characters.",
    );
  }
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new Error("Provide a valid HTTP or HTTPS site URL.");
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error("Provide a valid HTTP or HTTPS site URL.");
  }
  return url.href;
}

/** Rescan commands use the named shell's literal argument syntax and a validated URL. */
export function formatRescanGuidance(
  sourceUrl: string,
  latest: ScanScore | null,
  platform: NodeJS.Platform = process.platform,
): string {
  const url = normalizeRescanUrl(sourceUrl);
  const windows = platform === "win32";
  const argument = windows ? `'${url.replace(/'/g, "''")}'` : `'${url.replace(/'/g, "'\"'\"'")}'`;
  const shell = windows ? "PowerShell" : "a POSIX shell (sh, bash, or zsh)";

  return [
    "## How to rescan a site",
    "",
    UNTRUSTED_DATA_INSTRUCTION,
    untrustedScanData(
      `Requested URL:\n${indentUntrustedEvidence(sourceUrl, MAX_RESCAN_URL_LENGTH)}`,
    ),
    "",
    "SiteCMD does not queue a scan from this tool. Pick one path:", // allow-machine-smell: negation, describes the guidance-only tool
    "",
    `CLI from the project folder, using ${shell}: run \`sitecmd scan\` to read .sitecmd/config.json. If that config is missing, initialize it once:`,
    "",
    `    sitecmd init ${argument}`,
    "",
    "To scan without a config:",
    "",
    `    sitecmd scan --url ${argument}`,
    "",
    "The CLI exports .sitecmd/ and syncs the desktop app when it is open.",
    "",
    "Desktop: open SiteCMD, select the project, and click Scan.",
    "",
    "After scanning, call `compare_scans` to see what was fixed, what is new, and what still fails.",
    "",
    latest
      ? `**Last scan:** ${describeScanAge(latest.timestamp, Date.now())}; web scan graded ${latest.overall_score}/100 with ${latest.issues_total} findings.`
      : "No previous scans found for this URL.",
  ].join("\n");
}
