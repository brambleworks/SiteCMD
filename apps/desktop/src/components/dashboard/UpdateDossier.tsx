import { useMemo } from "react";
import { IssueActionBar } from "@/components/issues/IssueActionBar";
import { FixWithAgentAction } from "@/components/issues/FixWithAgentAction";
import { useFixAttempt } from "@/components/issues/useFixAttempt";
import {
  DossierRail,
  IssueDossierPanel,
  type IssueDossierBadge,
} from "@/components/issues/IssueDossierPanel";
import { DossierRecentChangeRail } from "@/components/issues/DossierRailDetails";
import { useDesktopPromptCenter, type DesktopPromptEntry } from "@/lib/desktop-prompts";
import { getUpdateMemory } from "@/lib/update-memory";
import {
  buildPendingVerificationId,
  queuePendingVerification,
  resolvePendingVerification,
} from "@/lib/pending-verification";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import type { PackageUpdate } from "@/lib/types";
import { getPackageUpdateSourceLabel } from "@/lib/update-priority";
import { buildCommand, ECOSYSTEM_LABELS, getUpdateTargetVersion } from "./update-commands";
import {
  UpdateBestFirstFixSection,
  UpdateNoFixSection,
  UpdateSecurityAdvisorySection,
} from "./UpdateDossierSections";
import { buildUpdateAgentIssue, formatMemoryTime } from "./update-dossier-model";

export function UpdateDossier({
  update,
  allUpdates = [],
  projectId,
  url,
  projectPath,
  arrivalPrompt,
  onClose,
  onVerify,
  verifying,
}: {
  update: PackageUpdate;
  allUpdates?: PackageUpdate[];
  projectId: number;
  url: string;
  projectPath: string;
  arrivalPrompt?: DesktopPromptEntry | null;
  onClose: () => void;
  onVerify: () => void;
  verifying: boolean;
}) {
  const desktopPrompts = useDesktopPromptCenter();
  const memory = getUpdateMemory(projectPath, update);
  const command = buildCommand(update);
  const targetVersion = getUpdateTargetVersion(update);
  const normalizedUrl = useMemo(() => normalizeAppUrlForKey(url), [url]);
  // Null for minor/patch updates: those never become work items, so there is
  // no issue identity the fix-attempt machinery could verify against.
  const agentIssue = useMemo(() => buildUpdateAgentIssue(update, allUpdates), [update, allUpdates]);
  // Update kinds share lifecycle identities across the dashboard and Issues.
  const lifecycleCheckId =
    agentIssue?.checkId ??
    (update.isSecurity ? "dependencies.vulnerability" : "dependencies.outdated-major");
  const { attempt, setAttempt } = useFixAttempt({
    projectId: agentIssue ? projectId : null,
    envUrl: normalizedUrl,
    checkId: agentIssue?.checkId ?? "",
    title: agentIssue?.title ?? update.name,
  });
  const likelyChangedPrompt = useMemo(() => {
    if (
      arrivalPrompt &&
      arrivalPrompt.projectId === projectId &&
      arrivalPrompt.page === "updates" &&
      normalizeAppUrlForKey(arrivalPrompt.url) === normalizedUrl
    ) {
      return arrivalPrompt;
    }
    return (
      desktopPrompts.find(
        (entry) => entry.page === "updates" && entry.absolutePath?.startsWith(projectPath),
      ) ?? null
    );
  }, [arrivalPrompt, desktopPrompts, normalizedUrl, projectId, projectPath]);
  const badges: IssueDossierBadge[] = [
    {
      label: update.isSecurity ? "Security" : update.updateType,
      tone: update.isSecurity ? "critical" : update.updateType === "major" ? "warning" : "info",
    },
    { label: ECOSYSTEM_LABELS[update.ecosystem], tone: "muted" },
    { label: update.isDev ? "Dev dependency" : "Production dependency", tone: "muted" },
  ];
  const queueUpdatePending = (reason: string, filePath?: string | null) => {
    queuePendingVerification({
      projectId,
      url,
      itemId: `${update.ecosystem}:${update.name}`,
      label: update.name,
      reason,
      page: "updates",
      filePath: filePath ?? likelyChangedPrompt?.absolutePath ?? null,
    });
  };

  // Leave this short chain to React Compiler rather than pinning manual dependencies.
  const memoryStatus = memory?.regressedAfterVerifiedAt
    ? { label: "Regressed", value: formatMemoryTime(memory.regressedAfterVerifiedAt) }
    : memory?.lastVerifiedAt
      ? { label: "Last verified", value: formatMemoryTime(memory.lastVerifiedAt) }
      : memory?.firstSeenAt
        ? { label: "First seen", value: formatMemoryTime(memory.firstSeenAt) }
        : null;

  const leftRail = (
    <>
      <DossierRail label="Update source">
        <p className="dossier-rail-mono">{getPackageUpdateSourceLabel(update)}</p>
      </DossierRail>

      {memoryStatus ? (
        <DossierRail label="History">
          <div className="dossier-rail-list">
            <div className="dossier-rail-row">
              <span className="dossier-rail-row-key">{memoryStatus.label}</span>
              <span className="dossier-rail-row-value">{memoryStatus.value}</span>
            </div>
          </div>
        </DossierRail>
      ) : null}
    </>
  );

  const rightRail = (
    <>
      <DossierRail>
        <IssueActionBar
          className="dossier-actions"
          projectId={projectId}
          checkId={lifecycleCheckId}
          envUrl={normalizedUrl}
          verifyAction={{
            label: "Verify",
            onClick: onVerify,
            verifying,
          }}
          extraActions={
            agentIssue ? (
              <FixWithAgentAction
                projectId={projectId}
                envUrl={normalizedUrl}
                checkId={agentIssue.checkId}
                title={agentIssue.title}
                severity={agentIssue.severity}
                description={agentIssue.description}
                url={url}
                whyItMatters={agentIssue.whyItMatters}
                evidence={agentIssue.evidence}
                manualFix={agentIssue.manualFix}
                previousFailure={attempt?.status === "verify_failed" ? attempt.failureDetail : null}
                projectPath={projectPath}
                onAttemptCreated={setAttempt}
              />
            ) : undefined
          }
          onIgnore={() => {
            resolvePendingVerification(
              buildPendingVerificationId(
                projectId,
                normalizedUrl,
                `${update.ecosystem}:${update.name}`,
                "updates",
              ),
            );
          }}
          onBlock={() => {
            resolvePendingVerification(
              buildPendingVerificationId(
                projectId,
                normalizedUrl,
                `${update.ecosystem}:${update.name}`,
                "updates",
              ),
            );
          }}
        />
      </DossierRail>
      {likelyChangedPrompt ? <DossierRecentChangeRail prompt={likelyChangedPrompt} /> : null}
    </>
  );

  return (
    <IssueDossierPanel
      title={update.name}
      subtitle={
        targetVersion
          ? `${update.currentVersion} -> ${targetVersion}`
          : `${update.currentVersion} (no fixed release)`
      }
      eyebrow="Details"
      badges={badges}
      leftRail={leftRail}
      rightRail={rightRail}
      onClose={onClose}>
      {update.isSecurity ? <UpdateSecurityAdvisorySection update={update} /> : null}

      {command ? (
        <UpdateBestFirstFixSection
          command={command}
          onCopy={() => queueUpdatePending("Copied dependency command")}
        />
      ) : update.isSecurity ? (
        <UpdateNoFixSection />
      ) : null}
    </IssueDossierPanel>
  );
}
