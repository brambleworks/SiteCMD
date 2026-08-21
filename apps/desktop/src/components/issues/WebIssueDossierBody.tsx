import type { CheckResult, FixLocation, IssueGroup } from "@/lib/types";
import { Markdown } from "@/components/ui/markdown";
import { Button } from "@/components/ui/button";
import { AsyncFixGuideSteps } from "@/components/ui/AsyncFixGuideSteps";
import { DossierKeyValueGrid } from "@/components/issues/IssueDossierPanel";
import { RootCauseCallout } from "@/components/issues/RootCauseCallout";
import { IntegrationHintCallout } from "@/components/issues/IntegrationHintCallout";
import {
  IssueHowToFixSection,
  IssueProofSection,
  IssueV3Footer,
  IssueV3HeaderExtras,
  IssueWhatSection,
  IssueWhereLivesSection,
  ProofBlock,
  type IssueAffectedFile,
  type IssueAffectedPage,
} from "@/components/issues/IssueDossierSections";
import { DossierSectionTabs } from "@/components/issues/DossierSectionTabs";
import { EnrichmentSection } from "@/components/issues/EnrichmentSection";
import { formatUrlPathOrHost } from "@/lib/utils";
import { coerceJsonRecord } from "@/lib/json-record";
import {
  formatRelativeDate,
  labelForSource,
  summarizeEvidence,
} from "@/components/dashboard/dashboard-issue-evidence";
import { useCurrentTime } from "@/lib/useCurrentTime";

function buildAffectedPages(
  group: IssueGroup | undefined,
  groupedOccurrenceLabels: string[],
  fallbackUrl: string,
): IssueAffectedPage[] {
  if (group?.instances && group.instances.length > 0) {
    const seen = new Set<string>();
    const rows: IssueAffectedPage[] = [];
    for (const inst of group.instances) {
      const candidate = inst.url ?? inst.signalId;
      if (!candidate || seen.has(candidate)) continue;
      seen.add(candidate);
      const label = formatUrlPathOrHost(candidate, candidate);
      rows.push({ key: String(inst.id), label, lastSeen: inst.lastSeenAt });
    }
    if (rows.length > 0) return rows;
  }
  if (groupedOccurrenceLabels.length > 0) {
    return groupedOccurrenceLabels.map((label, i) => ({ key: `${label}-${i}`, label }));
  }
  const fallbackLabel = formatUrlPathOrHost(fallbackUrl, fallbackUrl);
  return [{ key: fallbackUrl, label: fallbackLabel }];
}

function toAffectedFiles(files: FixLocation[]): IssueAffectedFile[] {
  return files.map((file) => ({
    key: file.absolutePath,
    label: file.label,
    reason: file.reason,
    relativePath: file.relativePath,
  }));
}

interface WebIssueRichSectionsProps {
  issue: CheckResult;
  /** Stack the scan detected, forwarded to guide resolution so the catalog's
   *  matching variant is preferred over the generic default steps. */
  detectedStack?: Record<string, unknown> | null;
  group?: IssueGroup;
  groupedOccurrenceLabels: string[];
  locationCount: number;
  pageUrl: string;
  primaryCorrelatedFile: FixLocation | null;
  correlatedFiles: FixLocation[];
  fixText: string;
  projectId: number;
  projectPath: string | null;
  verifying: boolean;
  onOpenEditor: () => Promise<void>;
  onVerifyFor: (src: string) => Promise<void>;
  onOpenFile: (file: FixLocation) => Promise<void> | void;
  onRevealFile: (file: FixLocation) => Promise<void> | void;
  onOpenCauseDossier?: (checkId: string) => void;
  onOpenIntegrations?: (integration: string) => void;
}

export function WebIssueRichSections({
  issue,
  detectedStack = null,
  group,
  groupedOccurrenceLabels,
  locationCount,
  pageUrl,
  primaryCorrelatedFile,
  correlatedFiles,
  fixText,
  projectId,
  projectPath,
  verifying,
  onOpenEditor,
  onVerifyFor,
  onOpenFile,
  onRevealFile,
  onOpenCauseDossier,
  onOpenIntegrations,
}: WebIssueRichSectionsProps) {
  const nowMs = useCurrentTime();
  const affectedPages = buildAffectedPages(group, groupedOccurrenceLabels, pageUrl);
  const affectedFiles = toAffectedFiles(correlatedFiles);
  const fileKeyToFixLocation = new Map(correlatedFiles.map((f) => [f.absolutePath, f]));

  const hasMultiSourceEvidence = Boolean(group && group.sources.length > 1);
  const hasLikelyCauses = Boolean(group?.likelyCauses && group.likelyCauses.length > 0);
  const hasIntegrationHints = Boolean(
    group?.suggestedIntegrations && group.suggestedIntegrations.length > 0,
  );
  const hasRawEvidence = Boolean(issue.rawData && Object.keys(issue.rawData).length > 0);
  const hasProofContent =
    hasMultiSourceEvidence || hasLikelyCauses || hasIntegrationHints || hasRawEvidence;
  const proofSummary = "Supporting evidence captured by SiteCMD.";

  const enrichments = group?.enrichments ?? [];

  return (
    <>
      {group ? <IssueV3HeaderExtras issue={group} /> : null}

      <DossierSectionTabs
        tabs={[
          {
            label: "Description",
            content: <IssueWhatSection description={issue.description} />,
          },
          {
            label: `Locations (${locationCount})`,
            content: (
              <IssueWhereLivesSection
                pages={affectedPages}
                files={affectedFiles}
                formatLastSeen={(timestamp) => formatRelativeDate(timestamp, nowMs)}
                onOpenFile={(file) => {
                  const original = fileKeyToFixLocation.get(file.key);
                  if (original) void onOpenFile(original);
                }}
                onRevealFile={(file) => {
                  const original = fileKeyToFixLocation.get(file.key);
                  if (original) void onRevealFile(original);
                }}
              />
            ),
          },
          {
            label: "How to fix",
            content: (
              <IssueHowToFixSection>
                <AsyncFixGuideSteps
                  kind="web"
                  checkId={issue.checkId}
                  detectedStack={detectedStack}
                  fallback={
                    fixText ? (
                      <Markdown>{fixText}</Markdown>
                    ) : (
                      <p className="body-text-muted">No automated fix plan yet.</p>
                    )
                  }
                />
                {projectPath && primaryCorrelatedFile ? (
                  <Button
                    variant="ghost"
                    className="dossier-open-file-btn text-meta"
                    onClick={onOpenEditor}>
                    Open {primaryCorrelatedFile.relativePath}
                  </Button>
                ) : null}
                {group && group.sources.length > 1 ? (
                  <div className="verify-actions">
                    {group.sources.map((src) => (
                      <Button
                        key={src}
                        variant="secondary"
                        size="sm"
                        onClick={() => onVerifyFor(src)}
                        disabled={verifying}>
                        Verify with {labelForSource(src)}
                      </Button>
                    ))}
                  </div>
                ) : null}
              </IssueHowToFixSection>
            ),
          },
          {
            label: "Evidence",
            content: hasProofContent ? (
              <IssueProofSection summary={proofSummary}>
                {hasMultiSourceEvidence ? (
                  <ProofBlock>
                    <ul className="evidence-list">
                      {group!.sources.map((src) => (
                        <li key={src} className="evidence-row">
                          <span className="evidence-source-label">{labelForSource(src)}</span>
                          <span className="evidence-detail">
                            {summarizeEvidence(group!, src, nowMs)}
                          </span>
                        </li>
                      ))}
                    </ul>
                  </ProofBlock>
                ) : null}

                {hasLikelyCauses ? (
                  <ProofBlock>
                    <RootCauseCallout
                      causes={group!.likelyCauses!}
                      onOpenCause={(checkId) => onOpenCauseDossier?.(checkId)}
                    />
                  </ProofBlock>
                ) : null}

                {hasIntegrationHints ? (
                  <ProofBlock>
                    <IntegrationHintCallout
                      projectId={projectId}
                      suggestions={group!.suggestedIntegrations!}
                      onOpenIntegrations={onOpenIntegrations}
                      onDismissed={() => {
                        // Backend dismissal already happened inside the component.
                      }}
                    />
                  </ProofBlock>
                ) : null}

                {hasRawEvidence ? (
                  <ProofBlock>
                    <DossierKeyValueGrid data={coerceJsonRecord(issue.rawData) ?? {}} />
                  </ProofBlock>
                ) : null}
              </IssueProofSection>
            ) : null,
          },
        ]}
      />

      {enrichments.length > 0 ? <EnrichmentSection enrichments={enrichments} /> : null}

      {group ? <IssueV3Footer issue={group} /> : null}
    </>
  );
}
