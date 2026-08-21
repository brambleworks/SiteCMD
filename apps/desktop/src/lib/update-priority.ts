import { normalizeAppUrlForKey, type AppTarget } from "@/lib/app-targets";
import type { PackageUpdate } from "@/lib/types";

export function isCriticalSecurityUpdate(
  update: Pick<PackageUpdate, "isSecurity" | "advisorySeverity">,
): boolean {
  return update.isSecurity && (update.advisorySeverity ?? "").toLowerCase() === "critical";
}

export function getPackageUpdateTargetVersion(
  update: Pick<PackageUpdate, "latestVersion" | "isSecurity" | "advisoryFixedVersion">,
): string | null {
  const target = update.isSecurity ? update.advisoryFixedVersion : update.latestVersion;
  return target?.trim() || null;
}

function getUpdateSeverityRank(severity: string | null | undefined): number {
  switch ((severity ?? "").toLowerCase()) {
    case "critical":
      return 0;
    case "high":
      return 1;
    case "moderate":
    case "medium":
      return 2;
    case "low":
      return 3;
    default:
      return 4;
  }
}

function getUpdatePriorityRank(update: PackageUpdate): number {
  if (update.isSecurity) {
    return getUpdateSeverityRank(update.advisorySeverity);
  }

  switch (update.updateType) {
    case "major":
      return 10;
    case "minor":
      return 20;
    case "patch":
      return 30;
    default:
      return 40;
  }
}

export function getPackageUpdateSourceLabel(
  update: Pick<PackageUpdate, "ecosystem" | "source">,
): string {
  const source = update.source?.trim();
  if (!source || source.toLowerCase() === update.ecosystem.toLowerCase()) {
    return `${update.ecosystem} dependency surface`;
  }
  return source;
}

export function formatPackageUpdateSummary(update: PackageUpdate): string {
  const urgency = update.isSecurity
    ? `security${update.advisorySeverity ? ` (${update.advisorySeverity})` : ""}`
    : update.updateType;
  const target = getPackageUpdateTargetVersion(update);
  const versionChange = target
    ? `${update.currentVersion} -> ${target}`
    : `${update.currentVersion} (no fixed release)`;
  return `${update.name} ${versionChange} • ${urgency}`;
}

function comparePackageUpdates(a: PackageUpdate, b: PackageUpdate): number {
  const aPriority = getUpdatePriorityRank(a);
  const bPriority = getUpdatePriorityRank(b);
  if (aPriority !== bPriority) return aPriority - bPriority;

  const aProd = a.isDev ? 1 : 0;
  const bProd = b.isDev ? 1 : 0;
  if (aProd !== bProd) return aProd - bProd;

  const sourceOrder = a.source.localeCompare(b.source);
  if (sourceOrder !== 0) return sourceOrder;

  return a.name.localeCompare(b.name);
}

function compareRelatedPackageUpdates(
  primary: PackageUpdate,
  a: PackageUpdate,
  b: PackageUpdate,
): number {
  const aSameSource = a.source === primary.source ? 0 : 1;
  const bSameSource = b.source === primary.source ? 0 : 1;
  if (aSameSource !== bSameSource) return aSameSource - bSameSource;

  return comparePackageUpdates(a, b);
}

export function findStrongestPackageUpdate(updates: PackageUpdate[]): PackageUpdate | null {
  return [...updates].sort(comparePackageUpdates)[0] ?? null;
}

export function findNextRelatedUpdate(
  primary: PackageUpdate,
  candidates: PackageUpdate[],
): PackageUpdate | null {
  return (
    candidates
      .filter(
        (candidate) =>
          !(candidate.ecosystem === primary.ecosystem && candidate.name === primary.name),
      )
      .sort((a, b) => compareRelatedPackageUpdates(primary, a, b))[0] ?? null
  );
}

interface UpdateCampaignCopyInput {
  totalCount: number;
  securityCount?: number | null;
  leadLabel: string;
  leadSummary?: string | null;
  leadSourceLabel?: string | null;
  mode?: "fix" | "verify" | "resume";
}

export function buildUpdateCampaignCopy(input: UpdateCampaignCopyInput): {
  title: string;
  detail: string;
} {
  const totalCount = Math.max(0, input.totalCount);
  const securityCount = Math.max(0, Math.min(input.securityCount ?? 0, totalCount));
  const otherCount = Math.max(0, totalCount - securityCount);
  const remainingCount = Math.max(0, totalCount - 1);
  const lead = (input.leadSummary?.trim() || input.leadLabel).trim();
  const sourceSuffix = input.leadSourceLabel ? ` in ${input.leadSourceLabel}` : "";
  const mode = input.mode ?? "fix";

  if (mode === "resume") {
    return {
      title: `${totalCount} package ${totalCount === 1 ? "update came" : "updates came"} back`,
      detail:
        remainingCount > 0
          ? `Start in Updates with ${lead}${sourceSuffix}; ${remainingCount} more ${remainingCount === 1 ? "update also needs another look" : "updates also need another look"}.`
          : `Start in Updates with ${lead}${sourceSuffix} before this drifts any further.`,
    };
  }

  if (mode === "verify") {
    return {
      title: `${totalCount} package ${totalCount === 1 ? "update still needs" : "updates still need"} verification`,
      detail:
        remainingCount > 0
          ? `Start in Updates with ${lead}${sourceSuffix}; ${remainingCount} more dependency ${remainingCount === 1 ? "change still needs a quick check" : "changes still need a quick check"}.`
          : `Start in Updates with ${lead}${sourceSuffix} and make sure it behaves the way you expect before you move on.`,
    };
  }

  if (securityCount > 0) {
    return {
      title:
        otherCount > 0
          ? `${securityCount} vulnerable ${securityCount === 1 ? "package" : "packages"} and ${otherCount} other ${otherCount === 1 ? "update" : "updates"} still open`
          : `${securityCount} vulnerable ${securityCount === 1 ? "package is" : "packages are"} still open`,
      detail:
        remainingCount > 0
          ? `Start with ${lead}${sourceSuffix}; ${remainingCount} more ${remainingCount === 1 ? "package update is" : "package updates are"} already listed in Updates.`
          : `Start with ${lead}${sourceSuffix} to close out the vulnerable packages.`,
    };
  }

  return {
    title: `${totalCount} package ${totalCount === 1 ? "update is" : "updates are"} still open`,
    detail:
      remainingCount > 0
        ? `Start with ${lead}${sourceSuffix} so the rest of the dependency cleanup stays manageable.`
        : `Start with ${lead}${sourceSuffix} in Updates.`,
  };
}

export function buildPackageUpdateTarget(
  projectId: number,
  url: string,
  update: Pick<PackageUpdate, "ecosystem" | "name">,
  extra?: Partial<AppTarget>,
): AppTarget {
  return {
    page: "updates",
    projectId,
    url: normalizeAppUrlForKey(url),
    itemId: `${update.ecosystem}:${update.name}`,
    ...extra,
  };
}
