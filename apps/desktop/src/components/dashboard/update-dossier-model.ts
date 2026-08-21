import type { PackageUpdate } from "@/lib/types";
import { buildCommand, ECOSYSTEM_LABELS, getUpdateTargetVersion } from "./update-commands";

export function formatMemoryTime(timestamp: number | null | undefined): string {
  if (!timestamp) return "-";
  return new Date(timestamp).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

/** One fix attempt covers every package in the selected update group. */
export interface UpdateAgentIssue {
  checkId: string;
  title: string;
  severity: string;
  description: string;
  whyItMatters: string;
  manualFix: string;
  evidence: Array<{
    package: string;
    ecosystem: string;
    current_version: string;
    latest_version: string;
    advisory_fixed_version?: string | null;
    advisory_severity?: string | null;
    advisory_url?: string | null;
  }>;
}

const ADVISORY_SEVERITY_RANK: Record<string, number> = {
  critical: 4,
  high: 3,
  moderate: 2,
  medium: 2,
  low: 1,
};

/** Return the highest normalized severity, defaulting unknown values to high. */
function groupAdvisorySeverity(group: PackageUpdate[]): string {
  let label = "high";
  let best = 0;
  for (const pkg of group) {
    const raw = (pkg.advisorySeverity ?? "").trim().toLowerCase();
    const effective = raw === "" || !(raw in ADVISORY_SEVERITY_RANK) ? "high" : raw;
    const rank = ADVISORY_SEVERITY_RANK[effective] ?? 3;
    if (rank > best) {
      best = rank;
      label = effective;
    }
  }
  return label === "moderate" ? "medium" : label;
}

function isVulnerabilityUpdate(update: PackageUpdate): boolean {
  return update.isSecurity;
}

/** Security takes precedence in the adapter, so a security major is part of
 * the vulnerability group, never the outdated-major group. */
function isMajorUpdate(update: PackageUpdate): boolean {
  return !update.isSecurity && update.updateType === "major";
}

/** Builds an agent-fix issue only for update kinds represented by work items. */
export function buildUpdateAgentIssue(
  update: PackageUpdate,
  allUpdates: PackageUpdate[],
): UpdateAgentIssue | null {
  const vulnerability = isVulnerabilityUpdate(update);
  if (!vulnerability && !isMajorUpdate(update)) return null;

  const sameKind = vulnerability ? isVulnerabilityUpdate : isMajorUpdate;
  // Selected package first; the Map dedupes it against allUpdates.
  const group = new Map<string, PackageUpdate>();
  group.set(`${update.ecosystem}:${update.name}`, update);
  for (const candidate of allUpdates) {
    if (sameKind(candidate)) {
      const key = `${candidate.ecosystem}:${candidate.name}`;
      if (!group.has(key)) group.set(key, candidate);
    }
  }
  const packages = [...group.values()];
  const targetVersion = getUpdateTargetVersion(update) ?? update.latestVersion;

  const ecosystemLabel = ECOSYSTEM_LABELS[update.ecosystem];
  const title = vulnerability
    ? packages.length === 1
      ? `Vulnerability in ${update.name} ${update.currentVersion} (${ecosystemLabel})`
      : `Vulnerabilities in ${packages.length} dependencies`
    : packages.length === 1
      ? `${update.name} has a major update (${update.currentVersion} -> ${targetVersion})`
      : `${packages.length} dependencies have major updates`;

  const intro = vulnerability
    ? "These installed packages have known security advisories. SiteCMD verifies this issue as a group, so remediate every package listed:"
    : "These dependencies are behind a major version. SiteCMD verifies this issue as a group, so update every package listed:";
  const packageLines = packages.map((pkg) => {
    const target = getUpdateTargetVersion(pkg);
    const version = target
      ? `${pkg.currentVersion} -> ${target}`
      : `${pkg.currentVersion} (no fixed release)`;
    return `- ${pkg.name} ${version} (${ECOSYSTEM_LABELS[pkg.ecosystem]})`;
  });
  const commands = packages
    .map(buildCommand)
    .filter((command): command is string => command !== null);
  const mitigations = vulnerability
    ? packages
        .filter((pkg) => getUpdateTargetVersion(pkg) === null)
        .map(
          (pkg) =>
            `For ${pkg.name}, determine reachability and remove, replace, or isolate the vulnerable dependency because no fixed release is published.`,
        )
    : [];

  return {
    checkId: vulnerability ? "dependencies.vulnerability" : "dependencies.outdated-major",
    title,
    severity: vulnerability ? groupAdvisorySeverity(packages) : "low",
    description: [intro, ...packageLines].join("\n"),
    whyItMatters: vulnerability
      ? "Every package listed has a published advisory against the installed version, leaving a known vulnerable dependency until it is remediated."
      : "Major versions left behind accumulate breaking-change debt, and the gap makes each future upgrade riskier.",
    manualFix: [...commands, ...mitigations].join("\n"),
    evidence: packages.map((pkg) => ({
      package: pkg.name,
      ecosystem: ECOSYSTEM_LABELS[pkg.ecosystem],
      current_version: pkg.currentVersion,
      latest_version: getUpdateTargetVersion(pkg) ?? pkg.latestVersion,
      ...(vulnerability
        ? {
            advisory_fixed_version: getUpdateTargetVersion(pkg),
            advisory_severity: pkg.advisorySeverity ?? null,
            advisory_url: pkg.advisoryUrl ?? null,
          }
        : {}),
    })),
  };
}
