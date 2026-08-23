import { useCallback, useMemo } from "react";
import type { CodeIssue, IssueGroup } from "@/lib/types";
import { getCodeScanDomainFocus, normalizeAppUrlForKey, type AppTarget } from "@/lib/app-targets";
import { CODE_SCAN_DOMAIN_META, getCodeIssueDomain } from "@/lib/code-scan-domains";
import { Markdown } from "@/components/ui/markdown";
import { DossierRail, IssueDossierPanel } from "@/components/issues/IssueDossierPanel";
import { formatSeverityLabel as toSeverityLabel, formatSeverityToneClass } from "@/lib/severity";
import { DossierVerifyCallout } from "@/components/issues/DossierStandardSections";
import {
  IssueHowToFixSection,
  IssueProofSection,
  IssueV3Footer,
  IssueV3HeaderExtras,
  IssueWhatSection,
  IssueWhereLivesSection,
  ProofBlock,
  type IssueAffectedFile,
} from "@/components/issues/IssueDossierSections";
import { DossierSectionTabs } from "@/components/issues/DossierSectionTabs";
import { EnrichmentSection } from "@/components/issues/EnrichmentSection";
import { IssueActionBar } from "@/components/issues/IssueActionBar";
import { FixWithAgentAction } from "@/components/issues/FixWithAgentAction";
import { useFixAttempt } from "@/components/issues/useFixAttempt";
import { IssueMemoryRail } from "@/components/issues/IssueMemorySection";
import { getGuardrailIssueScope } from "@/lib/issue-scope";
import { openPathInEditor, revealPath } from "@/lib/desktop-actions";
import {
  buildPendingVerificationId,
  queuePendingVerification,
  resolvePendingVerification,
} from "@/lib/pending-verification";
import { useToast } from "@/hooks/useToast";
import type { WorkItemStatus } from "@/lib/project-summary-types";
import { AsyncFixGuideSteps } from "@/components/ui/AsyncFixGuideSteps";
import { getIssueConfidence, getIssueConfidenceLabel } from "@/lib/issue-confidence";
import { DossierConfidenceRow } from "@/components/issues/DossierConfidenceRow";
import { userFacingError } from "@/lib/user-facing-error";

function codeIssueToFile(issue: CodeIssue, isPrimary: boolean): IssueAffectedFile {
  return {
    key: issue.id,
    label: isPrimary ? "Affected file" : "Same pattern",
    relativePath: issue.relativePath,
    locationSuffix: issue.line ? `:${issue.line}` : null,
  };
}

export function CodeIssueDossier({
  issue,
  group,
  groupedIssues,
  projectId,
  scanUrl,
  projectPath,
  framework,
  onVerify,
  verifying,
  onOpenIntegrations,
  onClose,
  onBack,
  lifecycleInitialStatus,
  onLifecycleIgnore,
  onLifecycleBlock,
  onLifecycleReopen,
}: {
  issue: CodeIssue;
  group?: IssueGroup;
  groupedIssues?: CodeIssue[];
  projectId: number;
  scanUrl: string;
  projectPath?: string | null;
  framework?: string | null;
  onVerify: () => void;
  verifying: boolean;
  onOpenTarget?: (target: AppTarget) => void;
  onOpenIntegrations?: () => void;
  onClose: () => void;
  onBack?: () => void;
  lifecycleInitialStatus?: WorkItemStatus | null;
  onLifecycleIgnore?: () => void | Promise<void>;
  onLifecycleBlock?: () => void | Promise<void>;
  onLifecycleReopen?: () => void | Promise<void>;
}) {
  const { success, error } = useToast();
  const effectiveGroupedIssues = useMemo(
    () => (groupedIssues?.length ? groupedIssues : [issue]),
    [groupedIssues, issue],
  );
  const scopeMeta = getGuardrailIssueScope(issue);
  const normalizedUrl = useMemo(() => normalizeAppUrlForKey(scanUrl), [scanUrl]);
  const { attempt, setAttempt } = useFixAttempt({
    projectId,
    envUrl: normalizedUrl,
    checkId: issue.checkId,
    title: issue.title,
  });
  const queueCodePending = useCallback(
    (reason: string, target: CodeIssue) => {
      if (!normalizedUrl) return;
      queuePendingVerification({
        projectId,
        url: normalizedUrl,
        itemId: target.id,
        label: target.title,
        reason,
        page: "issues",
        focus: getCodeScanDomainFocus(getCodeIssueDomain(target)),
        filePath: target.absolutePath,
      });
    },
    [normalizedUrl, projectId],
  );

  const handleOpenIssueInEditor = useCallback(
    async (target: CodeIssue) => {
      try {
        await openPathInEditor(target.absolutePath);
        queueCodePending("Opened code file from Code Scan", target);
        success("Opened in editor", target.relativePath);
      } catch (err) {
        error(
          "Could not open editor",
          userFacingError(
            err,
            "SiteCMD could not open your editor. Open the file yourself and paste the prompt.",
          ),
        );
      }
    },
    [error, queueCodePending, success],
  );

  const handleRevealIssueFile = useCallback(
    async (target: CodeIssue) => {
      try {
        await revealPath(target.absolutePath || projectPath || "");
        queueCodePending("Revealed code file from Code Scan", target);
        success("Revealed file", target.relativePath);
      } catch (err) {
        error(
          "Could not reveal file",
          userFacingError(
            err,
            "SiteCMD could not open it. Open the file from your editor instead.",
          ),
        );
      }
    },
    [error, projectPath, queueCodePending, success],
  );

  const domain = getCodeIssueDomain(issue);
  const domainLabel = CODE_SCAN_DOMAIN_META[domain]?.shortLabel ?? domain;
  const confidence = getIssueConfidence(issue);
  const confidenceLabel = getIssueConfidenceLabel(confidence);

  const affectedFiles = useMemo<IssueAffectedFile[]>(() => {
    const primary = codeIssueToFile(issue, true);
    if (effectiveGroupedIssues.length <= 1) return [primary];
    const seen = new Set<string>([primary.key]);
    const rows: IssueAffectedFile[] = [primary];
    for (const entry of effectiveGroupedIssues) {
      if (entry.id === issue.id || seen.has(entry.id)) continue;
      seen.add(entry.id);
      rows.push(codeIssueToFile(entry, false));
    }
    return rows;
  }, [effectiveGroupedIssues, issue]);

  const fileKeyToIssue = useMemo(() => {
    const map = new Map<string, CodeIssue>();
    map.set(issue.id, issue);
    for (const entry of effectiveGroupedIssues) {
      map.set(entry.id, entry);
    }
    return map;
  }, [effectiveGroupedIssues, issue]);

  const hasProofContent = Boolean(issue.sourceExcerpt);
  const proofSummary = "Source excerpt for the flagged location.";

  const showWhyItMatters = Boolean(issue.whyNow && issue.whyNow !== issue.description);

  const leftRail = (
    <>
      <DossierRail className="dossier-rail-section-plain">
        <div className="dossier-rail-list">
          <div className="dossier-rail-row">
            <span className="dossier-rail-row-key">Applies to</span>
            <span className="dossier-rail-row-value">{scopeMeta.scopeLabel}</span>
          </div>
          <DossierConfidenceRow label={confidenceLabel} reason={issue.confidenceReason} />
        </div>
      </DossierRail>
      <IssueMemoryRail
        projectId={projectId}
        url={scanUrl || ""}
        checkId={issue.checkId}
        currentStatus={lifecycleInitialStatus}
      />
    </>
  );

  const rightRail = (
    <>
      <DossierRail>
        <IssueActionBar
          className="dossier-actions"
          projectId={projectId}
          checkId={issue.checkId}
          envUrl={normalizedUrl}
          initialStatus={lifecycleInitialStatus}
          verifyAction={{
            label: "Verify",
            onClick: onVerify,
            verifying,
          }}
          extraActions={
            <FixWithAgentAction
              projectId={projectId}
              envUrl={normalizedUrl}
              checkId={issue.checkId}
              title={issue.title}
              severity={issue.severity}
              description={issue.description}
              url={scanUrl}
              whyItMatters={issue.whyNow}
              evidence={issue.evidence}
              manualFix={issue.likelyFix}
              previousFailure={attempt?.status === "verify_failed" ? attempt.failureDetail : null}
              projectPath={projectPath}
              onOpenIntegrations={onOpenIntegrations}
              onAttemptCreated={setAttempt}
              codeLocations={[
                {
                  label: "Flagged location",
                  path: issue.relativePath,
                  line: issue.line,
                  reason: "This is the exact location the scanner flagged.",
                },
              ]}
            />
          }
          onIgnore={async () => {
            await onLifecycleIgnore?.();
            resolvePendingVerification(
              buildPendingVerificationId(projectId, normalizedUrl, issue.id, "issues"),
            );
          }}
          onBlock={async () => {
            await onLifecycleBlock?.();
            resolvePendingVerification(
              buildPendingVerificationId(projectId, normalizedUrl, issue.id, "issues"),
            );
          }}
          onReopen={onLifecycleReopen ?? (() => {})}
        />
      </DossierRail>
    </>
  );

  const enrichments = group?.enrichments ?? [];

  return (
    <IssueDossierPanel
      title={issue.title}
      eyebrow={
        <>
          <span className={formatSeverityToneClass(issue.severity)}>
            {toSeverityLabel(issue.severity)}
          </span>
          {` - ${domainLabel}`}
        </>
      }
      subtitle={showWhyItMatters ? (issue.whyNow ?? undefined) : undefined}
      leftRail={leftRail}
      rightRail={rightRail}
      onClose={onClose}
      onBack={onBack}>
      {group ? <IssueV3HeaderExtras issue={group} /> : null}

      <DossierSectionTabs
        tabs={[
          {
            label: "Description",
            content: <IssueWhatSection description={issue.description} />,
          },
          {
            label: `Locations (${affectedFiles.length})`,
            content: (
              <IssueWhereLivesSection
                pages={[]}
                files={affectedFiles}
                filesPreamble={issue.evidence ?? null}
                onOpenFile={(file) => {
                  const target = fileKeyToIssue.get(file.key);
                  if (target) void handleOpenIssueInEditor(target);
                }}
                onRevealFile={(file) => {
                  const target = fileKeyToIssue.get(file.key);
                  if (target) void handleRevealIssueFile(target);
                }}
              />
            ),
          },
          {
            label: "How to fix",
            content: (
              <IssueHowToFixSection>
                <AsyncFixGuideSteps
                  kind="code"
                  checkId={issue.producerRuleId ?? ""}
                  framework={framework}
                  fallback={
                    issue.likelyFix ? (
                      <Markdown>{issue.likelyFix}</Markdown>
                    ) : (
                      <p className="body-text-muted">Isolate the risky code path in this file.</p>
                    )
                  }
                />
                {issue.verifyHint ? (
                  <DossierVerifyCallout>{issue.verifyHint}</DossierVerifyCallout>
                ) : null}
              </IssueHowToFixSection>
            ),
          },
          {
            label: "Evidence",
            content: hasProofContent ? (
              <IssueProofSection summary={proofSummary}>
                <ProofBlock>
                  <div className="card-sunken">
                    <pre className="compact-code-block">{issue.sourceExcerpt}</pre>
                  </div>
                </ProofBlock>
              </IssueProofSection>
            ) : (
              <p className="body-text-muted">No source excerpt was captured for this issue.</p>
            ),
          },
        ]}
      />

      {enrichments.length > 0 ? <EnrichmentSection enrichments={enrichments} /> : null}

      {group ? <IssueV3Footer issue={group} /> : null}
    </IssueDossierPanel>
  );
}
