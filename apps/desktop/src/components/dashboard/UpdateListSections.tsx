import { useState, type RefObject } from "react";
import { Check, ChevronRight, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { copyToClipboard } from "@/lib/clipboard";
import type { PackageUpdate } from "@/lib/types";
import { buildCommand, ECOSYSTEM_LABELS, getUpdateTargetVersion } from "./update-commands";
import { formatWorkspaceMembers } from "./updates-page-model";

export type UpdateFilter = "all" | "major" | "minor" | "patch";

export function UpdateFilterPills({
  sectionRef,
  filter,
  counts,
  onFilterChange,
}: {
  sectionRef: RefObject<HTMLDivElement | null>;
  filter: UpdateFilter;
  counts: Record<UpdateFilter, number>;
  onFilterChange: (filter: UpdateFilter) => void;
}) {
  return (
    <div ref={sectionRef} className="row">
      {[
        { key: "all" as const, label: `All (${counts.all})` },
        { key: "major" as const, label: `Major (${counts.major})` },
        { key: "minor" as const, label: `Minor (${counts.minor})` },
        { key: "patch" as const, label: `Patch (${counts.patch})` },
      ].map((item) => (
        <Button
          unstyled
          key={item.key}
          onClick={() => onFilterChange(item.key)}
          className={`update-filter-pill ghost-border ${
            filter === item.key ? "update-filter-pill--active" : "update-filter-pill--inactive"
          }`}>
          {item.label}
        </Button>
      ))}
    </div>
  );
}

export function UpdatesRowsSkeleton() {
  return (
    <LoadingRegion label="Update results loading" className="stack-snug">
      {[1, 2, 3, 4, 5].map((index) => (
        <div key={index} className="dashboard-loading-row">
          <Skeleton className="update-skeleton-tag" />
          <div className="update-skeleton-lines">
            <Skeleton className="update-skeleton-name" />
            <Skeleton className="update-skeleton-sub" />
          </div>
        </div>
      ))}
    </LoadingRegion>
  );
}

export function UpdateSection({
  label,
  color,
  updates,
  onOpenDossier,
}: {
  label: string;
  color: string;
  updates: PackageUpdate[];
  onOpenDossier: (update: PackageUpdate) => void;
}) {
  const [showAll, setShowAll] = useState(false);
  const INITIAL_SHOW = 5;
  const visible = showAll ? updates : updates.slice(0, INITIAL_SHOW);
  const remaining = updates.length - INITIAL_SHOW;

  return (
    <div>
      <div className="updates-section-head">
        <span className={`update-section-label ${color}`}>{label}</span>
        <div className="update-section-rule" />
      </div>

      <div className="panel panel--flush panel--muted">
        {visible.map((update, index) => (
          <div
            key={`${update.ecosystem}-${update.name}`}
            className={index > 0 ? "subtle-divider-top" : undefined}>
            <UpdateRow update={update} onOpenDossier={() => onOpenDossier(update)} />
          </div>
        ))}
        {!showAll && remaining > 0 && (
          <Button
            unstyled
            onClick={() => setShowAll(true)}
            className="subtle-divider-top update-show-more section-label-mid">
            Show {remaining} more {remaining === 1 ? "update" : "updates"}
          </Button>
        )}
      </div>
    </div>
  );
}

function UpdateRow({
  update,
  onOpenDossier,
}: {
  update: PackageUpdate;
  onOpenDossier: () => void;
}) {
  // In a monorepo "better-sqlite3 12 -> 13" is ambiguous without the member
  // that declares it; outside one this is null and the row is unchanged.
  const workspaceMembers = formatWorkspaceMembers(update.workspaceMembers);
  return (
    <Button
      unstyled
      type="button"
      data-dossier-switch="true"
      onClick={onOpenDossier}
      // Without this the row announces as "reactnpm18.2.0>19.2.7": the row text
      // has no whitespace between its parts, so the computed name runs together.
      aria-label={`${update.name}, update from ${update.currentVersion} to ${update.latestVersion}${
        workspaceMembers ? `, in ${workspaceMembers}` : ""
      }`}
      className="list-row-hover row-button">
      <div className="flex-fill">
        <p className="list-row__title row-title-lg text-mono text-truncate">{update.name}</p>
        {workspaceMembers && (
          <p className="subtitle-xs text-truncate text-mono text-muted-foreground update-row-note">
            {workspaceMembers}
          </p>
        )}
        {update.advisorySeverity && (
          <p className="subtitle-xs text-truncate update-row-note">
            Advisory: {update.advisorySeverity}
          </p>
        )}
        {update.isDeprecated && (
          <p className="subtitle-xs text-truncate text-severity-medium update-row-note">
            Deprecated by maintainer
            {update.deprecationMessage ? `: ${update.deprecationMessage}` : ""}
          </p>
        )}
        {!update.isDeprecated && update.isStale && (
          <p className="subtitle-xs text-truncate text-muted-foreground update-row-note">
            No releases in 3+ years
            {update.lastPublished ? ` (last: ${update.lastPublished.slice(0, 10)})` : ""}
          </p>
        )}
      </div>

      <div className="update-version-cell text-body">
        <span className="text-muted-foreground text-mono">{update.currentVersion}</span>
        <span className="text-muted-foreground">&rarr;</span>
        <span className="text-foreground update-version-new text-mono">{update.latestVersion}</span>
      </div>

      <ChevronRight className="list-row__chevron icon-md" aria-hidden="true" />
    </Button>
  );
}

export function SecurityBanner({
  updates,
  onOpenDossier,
}: {
  updates: PackageUpdate[];
  onOpenDossier: (update: PackageUpdate) => void;
}) {
  // No copy-all control here: the page header's "Copy All Commands" already
  // covers it, and two copy-everything buttons on one screen invite the wrong one.
  return (
    <div>
      <div className="updates-section-head">
        <span className="update-section-label text-severity-critical">
          SECURITY UPDATES ({updates.length})
        </span>
        <div className="update-section-rule" />
      </div>
      <div className="panel panel--flush panel--muted">
        {updates.map((update, index) => (
          <SecurityRow
            key={`${update.ecosystem}-${update.name}`}
            update={update}
            showBorder={index > 0}
            onOpenDossier={() => onOpenDossier(update)}
          />
        ))}
      </div>
    </div>
  );
}

function SecurityRow({
  update,
  showBorder,
  onOpenDossier,
}: {
  update: PackageUpdate;
  showBorder: boolean;
  onOpenDossier: () => void;
}) {
  const targetVersion = getUpdateTargetVersion(update);

  return (
    <Button
      unstyled
      type="button"
      data-dossier-switch="true"
      onClick={onOpenDossier}
      aria-label={`${update.name}, security advisory for ${update.currentVersion}, ${
        targetVersion ? `fixed in ${targetVersion}` : "no fixed release available"
      }`}
      className={`list-row-hover row-button ${showBorder ? "subtle-divider-top" : ""}`}>
      <div className="row flex-fill">
        <span className="list-row__title row-title-md text-mono text-truncate">{update.name}</span>
        <span className="subtitle-xs no-shrink">{ECOSYSTEM_LABELS[update.ecosystem]}</span>
      </div>
      <div className="update-version-cell text-body-muted">
        <span className="text-muted-foreground text-mono">{update.currentVersion}</span>
        <span className="text-muted-foreground">&rarr;</span>
        {targetVersion ? (
          <span className="text-score-excellent update-version-new text-mono">{targetVersion}</span>
        ) : (
          <span className="text-meta text-severity-critical text-italic">no fix</span>
        )}
      </div>
      <ChevronRight className="list-row__chevron icon-md" aria-hidden="true" />
    </Button>
  );
}

export function CopyAllButton({ updates, label }: { updates: PackageUpdate[]; label: string }) {
  const [copied, setCopied] = useState(false);
  const commands = updates
    .map(buildCommand)
    .filter((command): command is string => command !== null);
  const copyAll = async () => {
    if (commands.length === 0) return;
    await copyToClipboard(commands.join("\n"));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  return (
    <Button onClick={copyAll} size="sm" disabled={commands.length === 0} className="btn--gap-snug">
      {copied ? (
        <>
          <Check className="icon-md" /> Copied
        </>
      ) : (
        <>
          <Copy className="icon-md" /> {label}
        </>
      )}
    </Button>
  );
}
