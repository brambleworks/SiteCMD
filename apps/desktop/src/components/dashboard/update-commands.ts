import type { Ecosystem, PackageUpdate } from "@/lib/types";
import { getPackageUpdateTargetVersion } from "@/lib/update-priority";

export const ECOSYSTEM_LABELS: Record<Ecosystem, string> = {
  npm: "npm",
  composer: "Composer",
  wordpress: "WordPress",
  drupal: "Drupal",
  python: "Python",
  ruby: "Ruby",
  go: "Go",
  rust: "Rust",
};

type UpdateCommandInput = Pick<
  PackageUpdate,
  "name" | "latestVersion" | "isSecurity" | "advisoryFixedVersion"
> & {
  ecosystem: Ecosystem | string;
};

export function getUpdateTargetVersion(update: UpdateCommandInput): string | null {
  return getPackageUpdateTargetVersion(update);
}

export function buildCommand(update: UpdateCommandInput): string | null {
  const target = getUpdateTargetVersion(update);
  if (!target) return null;

  switch (update.ecosystem) {
    case "npm":
      return `npm install ${update.name}@${target}`;
    case "composer":
      return `composer require ${update.name}:${target}`;
    case "wordpress":
      return `wp plugin update ${update.name}`;
    case "drupal":
      return `composer require drupal/${update.name}:${target}`;
    case "python":
      return `pip install ${update.name}==${target}`;
    case "ruby":
      return `bundle update ${update.name}`;
    case "go":
      return `go get ${update.name}@v${target}`;
    case "rust":
      return `cargo update -p ${update.name}`;
    default:
      return `update ${update.name} to ${target}`;
  }
}

export function buildAiTask(update: PackageUpdate): string {
  const target = getUpdateTargetVersion(update);
  if (update.isSecurity && !target) {
    return [
      `Help me respond to the security advisory for ${update.name} in this project.`,
      "",
      `Advisory severity: ${update.advisorySeverity || "unknown"}`,
      `Current version: ${update.currentVersion}`,
      "Fixed release: not published",
      `Ecosystem: ${ECOSYSTEM_LABELS[update.ecosystem]}`,
      "",
      "Please:",
      "- determine whether the vulnerable code path is reachable",
      "- recommend bounded mitigations or a replacement package",
      "- explain how to monitor for a fixed release",
      "- end with verification steps for the mitigation",
    ].join("\n");
  }

  const riskLine = update.isSecurity
    ? `This is a security update with advisory severity ${update.advisorySeverity || "unknown"}.`
    : `This is a ${update.updateType} dependency update.`;
  return [
    `Help me update ${update.name} in this project.`,
    "",
    riskLine,
    `Current version: ${update.currentVersion}`,
    `Target version: ${target}`,
    `Ecosystem: ${ECOSYSTEM_LABELS[update.ecosystem]}`,
    "",
    "Please:",
    "- explain the safest upgrade path",
    "- point out likely breaking changes or migration notes",
    "- give the exact command(s) to run",
    "- end with the verification steps I should run after upgrading",
  ].join("\n");
}
