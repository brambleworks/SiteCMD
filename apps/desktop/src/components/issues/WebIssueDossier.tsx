import { useMemo, useState } from "react";
import { coerceJsonRecord } from "@/lib/json-record";
import type { CheckResult, IssueGroup } from "@/lib/types";
import { verifyIssue } from "@/lib/issues";
import { useToast } from "@/hooks/useToast";
import { DossierRail, IssueDossierPanel } from "@/components/issues/IssueDossierPanel";
import { formatSeverityLabel as toSeverityLabel, formatSeverityToneClass } from "@/lib/severity";
import { pickSupportingDossierCopy } from "@/components/dashboard/dossier-copy";
import { FixWithAgentAction } from "@/components/issues/FixWithAgentAction";
import { SendToTrackerAction } from "@/components/issues/SendToTrackerAction";
import { useFixAttempt } from "@/components/issues/useFixAttempt";
import { IssueActionBar } from "@/components/issues/IssueActionBar";
import { IssueMemoryRail } from "@/components/issues/IssueMemorySection";
import { getCheckIssueScope } from "@/lib/issue-scope";
import { formatUrlPathOrHost } from "@/lib/utils";
import { CATEGORY_LABELS } from "@/lib/tokens";
import { getIssueConfidence, getIssueConfidenceLabel } from "@/lib/issue-confidence";
import { DossierConfidenceRow } from "@/components/issues/DossierConfidenceRow";
import { buildPendingVerificationId, resolvePendingVerification } from "@/lib/pending-verification";
import type { WorkItemStatus } from "@/lib/project-summary-types";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import {
  getIssuePageTarget,
  useIssueDossierActions,
} from "@/components/issues/useIssueDossierActions";
import { WebIssueRichSections } from "./WebIssueDossierBody";
import { userFacingError } from "@/lib/user-facing-error";

export function WebIssueDossier({
  issue,
  group,
  groupedIssues,
  projectId,
  url,
  projectPath,
  latestScanId = null,
  estimatedImpact = 0,
  detectedStack = null,
  onClose,
  onDismiss,
  onOpenCauseDossier,
  onOpenIntegrations,
  onIssueLinkCreated,
  onBack,
  lifecycleInitialStatus,
  onLifecycleIgnore,
  onLifecycleBlock,
  onLifecycleReopen,
  onLifecycleResolved,
}: {
  issue: CheckResult;
  group?: IssueGroup;
  groupedIssues?: CheckResult[];
  projectId: number;
  url: string;
  projectPath: string | null;
  /** Scan the dossier issue came from; ticket mirroring hides without it. */
  latestScanId?: number | null;
  /** Estimated score points this issue costs; goes into the mirrored ticket. */
  estimatedImpact?: number;
  /** Stack the scan detected, so remediation can prefer the matching catalog
   *  variant. Absent means default steps, never a guess. */
  detectedStack?: Record<string, unknown> | null;
  onClose: () => void;
  onDismiss?: (checkId: string) => void;
  onOpenCauseDossier?: (checkId: string) => void;
  onOpenIntegrations?: (integration: string) => void;
  onIssueLinkCreated?: () => void;
  onBack?: () => void;
  lifecycleInitialStatus?: WorkItemStatus | null;
  onLifecycleIgnore?: () => void | Promise<void>;
  onLifecycleBlock?: () => void | Promise<void>;
  onLifecycleReopen?: () => void | Promise<void>;
  onLifecycleResolved?: () => void | Promise<void>;
}) {
  const toast = useToast();
  const [verifying, setVerifying] = useState(false);
  const effectiveGroupedIssues = useMemo(
    () => (groupedIssues?.length ? groupedIssues : [issue]),
    [groupedIssues, issue],
  );
  const fixText = issue.fixPrompt || issue.manualFix || "";
  const scopeMeta = getCheckIssueScope(issue, url);
  const normalizedUrl = useMemo(() => normalizeAppUrlForKey(url), [url]);
  const { attempt, setAttempt } = useFixAttempt({
    projectId,
    envUrl: normalizedUrl,
    checkId: issue.checkId,
    title: issue.title,
  });
  const { page: pageTarget, focus: targetFocus } = getIssuePageTarget(issue);
  const {
    correlatedFiles,
    primaryCorrelatedFile,
    openEditor: handleOpenEditor,
    openFile: handleOpenLikelyFile,
    revealFile: handleRevealLikelyFile,
  } = useIssueDossierActions({
    issue,
    projectId,
    url,
    projectPath,
    page: pageTarget,
    focus: targetFocus,
    reasons: {
      openedPath: "Opened likely web fix file from Dashboard",
      revealedPath: "Revealed likely web fix file from Dashboard",
    },
  });
  const categoryLabel = CATEGORY_LABELS[issue.category] ?? issue.category;
  const whyItMattersCopy = pickSupportingDossierCopy(issue.description, [issue.whyItMatters]);
  const confidence = getIssueConfidence(issue);
  const confidenceLabel = getIssueConfidenceLabel(confidence);

  const groupedOccurrenceLabels = useMemo(() => {
    const labels = effectiveGroupedIssues.flatMap((entry) => {
      const raw = coerceJsonRecord(entry.rawData);
      if (!raw) return [];
      const candidate =
        raw.url ?? raw.pageUrl ?? raw.page_url ?? raw.path ?? raw.pathname ?? raw.route ?? null;
      if (typeof candidate !== "string" || !candidate.trim()) return [];
      return [formatUrlPathOrHost(candidate)];
    });
    return Array.from(new Set(labels)).slice(0, 6);
  }, [effectiveGroupedIssues]);
  const resolveFromDossier = async () => {
    if (onLifecycleResolved) {
      await onLifecycleResolved();
    }
    onDismiss?.(issue.checkId);
  };

  const handleVerify = async () => {
    setVerifying(true);
    try {
      const outcome = await verifyIssue(projectId, normalizedUrl, group?.checkId ?? issue.checkId);
      if (outcome.status === "verified") {
        await resolveFromDossier();
        toast.success("Verified", "A fresh check confirmed this issue is no longer detected.");
        onClose();
      } else if (outcome.status === "still_present") {
        toast.error("Still present", "A fresh check confirmed this issue is still detected.");
      } else {
        toast.success(
          "Verification started",
          "Integration-backed evidence is refreshing; the issue will update when polling finishes.",
        );
      }
    } catch (err) {
      toast.error(
        "Could not verify",
        userFacingError(err, "Run the verification again after the site has deployed."),
      );
    } finally {
      setVerifying(false);
    }
  };

  // The backend re-verifies all sources on the canonical group and returns an
  // explicit outcome. The source label is for UX context only.
  const handleVerifyFor = async (_src: string) => {
    if (!group) return;
    await handleVerify();
  };

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
        url={url}
        checkId={issue.checkId}
        currentStatus={issue.status}
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
            onClick: handleVerify,
            verifying,
          }}
          extraActions={
            <>
              <FixWithAgentAction
                projectId={projectId}
                envUrl={normalizedUrl}
                checkId={issue.checkId}
                title={issue.title}
                severity={issue.severity}
                description={issue.description}
                url={url}
                whyItMatters={issue.whyItMatters}
                evidence={issue.rawData}
                manualFix={issue.manualFix}
                previousFailure={attempt?.status === "verify_failed" ? attempt.failureDetail : null}
                projectPath={projectPath}
                onAttemptCreated={setAttempt}
                onOpenIntegrations={onOpenIntegrations ? () => onOpenIntegrations("") : undefined}
              />
              <SendToTrackerAction
                projectId={projectId}
                issue={issue}
                scanId={latestScanId}
                estimatedImpact={estimatedImpact}
                onLinkCreated={onIssueLinkCreated ? () => onIssueLinkCreated() : undefined}
              />
            </>
          }
          onIgnore={async () => {
            if (onLifecycleIgnore) {
              await onLifecycleIgnore();
            }
            onDismiss?.(issue.checkId);
            resolvePendingVerification(
              buildPendingVerificationId(projectId, normalizedUrl, issue.checkId, pageTarget),
            );
          }}
          onBlock={async () => {
            if (onLifecycleBlock) {
              await onLifecycleBlock();
            }
            onDismiss?.(issue.checkId);
            resolvePendingVerification(
              buildPendingVerificationId(projectId, normalizedUrl, issue.checkId, pageTarget),
            );
          }}
          onReopen={onLifecycleReopen ?? (() => {})}
        />
      </DossierRail>
    </>
  );

  return (
    <IssueDossierPanel
      title={issue.title}
      eyebrow={
        <>
          <span className={formatSeverityToneClass(issue.severity)}>
            {toSeverityLabel(issue.severity)}
          </span>
          {` - ${categoryLabel}`}
        </>
      }
      subtitle={whyItMattersCopy ?? undefined}
      leftRail={leftRail}
      rightRail={rightRail}
      onClose={onClose}
      onBack={onBack}>
      <WebIssueRichSections
        issue={issue}
        detectedStack={detectedStack}
        group={group}
        groupedOccurrenceLabels={groupedOccurrenceLabels}
        locationCount={Math.max(effectiveGroupedIssues.length, group?.instances.length ?? 0, 1)}
        pageUrl={normalizedUrl}
        primaryCorrelatedFile={primaryCorrelatedFile}
        correlatedFiles={correlatedFiles}
        fixText={fixText}
        projectId={projectId}
        projectPath={projectPath}
        verifying={verifying}
        onOpenEditor={handleOpenEditor}
        onVerifyFor={handleVerifyFor}
        onOpenFile={handleOpenLikelyFile}
        onRevealFile={handleRevealLikelyFile}
        onOpenCauseDossier={onOpenCauseDossier}
        onOpenIntegrations={onOpenIntegrations}
      />
    </IssueDossierPanel>
  );
}
