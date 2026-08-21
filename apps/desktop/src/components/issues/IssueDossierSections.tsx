import { FileText, Globe } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
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
          <div className="dossier-where-block">
            <ul className="dossier-where-list">
              {pages.map((page) => (
                <li key={page.key} className="dossier-where-row">
                  <Globe className="dossier-where-icon" aria-hidden="true" />
                  <span className="dossier-where-label" title={page.label}>
                    {page.label}
                  </span>
                  {page.lastSeen !== undefined && formatLastSeen ? (
                    <span className="dossier-where-meta">
                      last seen {formatLastSeen(page.lastSeen)}
                    </span>
                  ) : null}
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        {files.length > 0 ? (
          <div className="dossier-where-block">
            {filesPreamble ? <div className="dossier-where-preamble">{filesPreamble}</div> : null}
            <ul className="dossier-where-list">
              {files.map((file) => (
                <li key={file.key} className="dossier-where-row dossier-where-row-file">
                  <FileText className="dossier-where-icon" aria-hidden="true" />
                  <div className="dossier-where-file-text">
                    <p className="dossier-where-file-path" title={file.relativePath}>
                      {file.relativePath}
                      {file.locationSuffix ? (
                        <span className="dossier-where-file-suffix">{file.locationSuffix}</span>
                      ) : null}
                    </p>
                    {file.reason ? (
                      <p className="dossier-where-file-reason">{file.reason}</p>
                    ) : null}
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
          </div>
        ) : null}
      </div>
    </DossierNumberedSection>
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
