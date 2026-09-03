import { FileText, Globe } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Pager } from "@/components/ui/pager";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { pageWindow } from "@/lib/pagination";
import { DossierNumberedSection } from "@/components/issues/IssueDossierPanel";
import type { IssueGroup } from "@/lib/types";
import { AnomalyBadge } from "@/components/issues/AnomalyBadge";
import { CrossPageBadge } from "@/components/issues/CrossPageBadge";
import { TransitiveCauseCallout } from "@/components/issues/TransitiveCauseCallout";
import { CrossEnvCallout } from "@/components/issues/CrossEnvCallout";
import { CrossProjectLine } from "@/components/issues/CrossProjectLine";
import { ObservationFooter } from "@/components/issues/ObservationFooter";
import { EvidenceDisclosure } from "@/components/issues/EvidenceDisclosure";

export function IssueWhatSection({ description }: { description: string | null }) {
  return (
    <DossierNumberedSection label="Description" tone="attention">
      <p className="body-text">
        {description?.trim() || "No description was captured for this issue yet."}
      </p>
    </DossierNumberedSection>
  );
}

export interface IssueAffectedPage {
  key: string;
  label: string;
  lastSeen?: number;
}

export interface IssueAffectedFile {
  key: string;
  label: string;
  reason?: string | null;
  relativePath: string;
  locationSuffix?: string | null;
}

// An issue can hold thousands of locations; each block mounts one page of rows
// and the pager reveals the rest, as the Issues list does.
const LOCATION_PAGE_SIZE = 20;

export function IssueWhereLivesSection({
  pages,
  files,
  filesPreamble,
  formatLastSeen,
  onOpenFile,
  onRevealFile,
}: {
  pages: IssueAffectedPage[];
  files: IssueAffectedFile[];
  filesPreamble?: ReactNode;
  formatLastSeen?: (ts: number) => string;
  onOpenFile?: (file: IssueAffectedFile) => void;
  onRevealFile?: (file: IssueAffectedFile) => void;
}) {
  return (
    <DossierNumberedSection label="Location" tone="neutral">
      <div className="dossier-where">
        {pages.length > 0 ? (
          <AffectedPagesBlock pages={pages} formatLastSeen={formatLastSeen} />
        ) : null}

        {files.length > 0 ? (
          <AffectedFilesBlock
            files={files}
            filesPreamble={filesPreamble}
            onOpenFile={onOpenFile}
            onRevealFile={onRevealFile}
          />
        ) : null}
      </div>
    </DossierNumberedSection>
  );
}

function AffectedPagesBlock({
  pages,
  formatLastSeen,
}: {
  pages: IssueAffectedPage[];
  formatLastSeen?: (ts: number) => string;
}) {
  const [page, setPage] = useState(1);
  // A different issue, or a shorter list, changes what the first page holds.
  // Keyed on content rather than array identity: callers build these inline.
  useResetOnChange(`${pages.length}:${pages[0]?.key ?? ""}`, () => setPage(1));
  const bounded = pageWindow(pages, page, LOCATION_PAGE_SIZE);

  return (
    <div className="dossier-where-block">
      <ul className="dossier-where-list">
        {bounded.rows.map((location) => (
          <li key={location.key} className="dossier-where-row">
            <Globe className="dossier-where-icon" aria-hidden="true" />
            <span className="dossier-where-label" title={location.label}>
              {location.label}
            </span>
            {location.lastSeen !== undefined && formatLastSeen ? (
              <span className="dossier-where-meta">
                last seen {formatLastSeen(location.lastSeen)}
              </span>
            ) : null}
          </li>
        ))}
      </ul>
      <Pager
        page={bounded.page}
        totalPages={bounded.totalPages}
        onChange={setPage}
        label="Affected pages"
        itemLabel="location"
        className="dossier-where-pager"
      />
    </div>
  );
}

function AffectedFilesBlock({
  files,
  filesPreamble,
  onOpenFile,
  onRevealFile,
}: {
  files: IssueAffectedFile[];
  filesPreamble?: ReactNode;
  onOpenFile?: (file: IssueAffectedFile) => void;
  onRevealFile?: (file: IssueAffectedFile) => void;
}) {
  const [page, setPage] = useState(1);
  // A different issue, or a shorter list, changes what the first page holds.
  // Keyed on content rather than array identity: callers build these inline.
  useResetOnChange(`${files.length}:${files[0]?.key ?? ""}`, () => setPage(1));
  const bounded = pageWindow(files, page, LOCATION_PAGE_SIZE);

  return (
    <div className="dossier-where-block">
      {filesPreamble ? <div className="dossier-where-preamble">{filesPreamble}</div> : null}
      <ul className="dossier-where-list">
        {bounded.rows.map((file) => (
          <li key={file.key} className="dossier-where-row dossier-where-row-file">
            <FileText className="dossier-where-icon" aria-hidden="true" />
            <div className="dossier-where-file-text">
              <p className="dossier-where-file-path" title={file.relativePath}>
                {file.relativePath}
                {file.locationSuffix ? (
                  <span className="dossier-where-file-suffix">{file.locationSuffix}</span>
                ) : null}
              </p>
              {file.reason ? <p className="dossier-where-file-reason">{file.reason}</p> : null}
            </div>
            {onOpenFile || onRevealFile ? (
              <div className="dossier-where-file-actions">
                {onOpenFile ? (
                  <Button variant="outline" size="sm" onClick={() => onOpenFile(file)}>
                    Open
                  </Button>
                ) : null}
                {onRevealFile ? (
                  <Button variant="outline" size="sm" onClick={() => onRevealFile(file)}>
                    Reveal
                  </Button>
                ) : null}
              </div>
            ) : null}
          </li>
        ))}
      </ul>
      <Pager
        page={bounded.page}
        totalPages={bounded.totalPages}
        onChange={setPage}
        label="Affected files"
        itemLabel="location"
        className="dossier-where-pager"
      />
    </div>
  );
}

export function IssueHowToFixSection({ children }: { children: ReactNode }) {
  return (
    <DossierNumberedSection label="How to fix" tone="action">
      {children}
    </DossierNumberedSection>
  );
}

export function IssueProofSection({ summary, children }: { summary: string; children: ReactNode }) {
  return (
    <DossierNumberedSection label="Evidence" tone="supporting">
      <p className="body-muted">{summary}</p>
      <div className="stack-card">{children}</div>
    </DossierNumberedSection>
  );
}

export function ProofBlock({ children }: { children: ReactNode }) {
  return <div className="proof-block">{children}</div>;
}

export function IssueV3HeaderExtras({ issue }: { issue: IssueGroup }) {
  const transitives = issue.transitiveCauses ?? [];
  const affectedPages = issue.affectedPages ?? [];
  const anomalyScore = issue.anomalyScore ?? null;
  if (transitives.length === 0 && affectedPages.length <= 1 && anomalyScore == null) {
    return null;
  }
  return (
    <div className="dossier-v3-header-extras">
      {anomalyScore != null || affectedPages.length > 1 ? (
        <div className="dossier-v3-badge-row">
          <AnomalyBadge score={anomalyScore} />
          <CrossPageBadge pages={affectedPages} />
        </div>
      ) : null}
      <TransitiveCauseCallout causes={transitives} />
    </div>
  );
}

export function IssueV3Footer({ issue }: { issue: IssueGroup }) {
  const crossEnv = issue.crossEnvSignal ?? null;
  const crossProject = issue.crossProjectPattern ?? null;
  const observationCount = issue.observationCount ?? 0;
  const evidence = issue.correlationEvidence ?? [];
  if (!crossEnv && !crossProject && observationCount === 0 && evidence.length === 0) {
    return null;
  }
  return (
    <div className="dossier-v3-footer">
      {crossEnv ? <CrossEnvCallout signal={crossEnv} /> : null}
      {crossProject ? <CrossProjectLine pattern={crossProject} /> : null}
      {observationCount > 0 ? <ObservationFooter count={observationCount} /> : null}
      {evidence.length > 0 ? <EvidenceDisclosure evidence={evidence} /> : null}
    </div>
  );
}
