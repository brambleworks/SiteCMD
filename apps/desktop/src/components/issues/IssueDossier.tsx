import { Suspense, lazy, useCallback, useState } from "react";
import type { UnifiedFixIssue } from "@/lib/issue-ranking";
import type { PackageUpdate } from "@/lib/types";
import type { WorkItemStatus } from "@/lib/project-summary-types";
import { IssueDossierPanel } from "@/components/issues/IssueDossierPanel";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { verifyIssue } from "@/lib/issues";
import { useToast } from "@/hooks/useToast";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { userFacingError } from "@/lib/user-facing-error";

const WebIssueDossier = lazy(() =>
  import("@/components/dashboard/DashboardComponents").then((module) => ({
    default: module.WebIssueDossier,
  })),
);
const CodeIssueDossier = lazy(() =>
  import("@/components/scan/CodeScanResults").then((module) => ({
    default: module.CodeIssueDossier,
  })),
);
const AlertDetail = lazy(() =>
  import("@/components/issues/AlertDetail").then((module) => ({ default: module.AlertDetail })),
);

export interface IssueDossierProps {
  selected: UnifiedFixIssue | null;
  projectId: number;
  url: string;
  projectPath: string | null;
  /** Scan the displayed issues came from; ticket mirroring hides without it. */
  latestScanId?: number | null;
  /** Fired after an issue is mirrored to a tracker so link chips refresh. */
  onIssueLinkCreated?: () => void;
  securityUpdates?: PackageUpdate[];
  nonSecurityUpdates?: PackageUpdate[];
  lastCIRun?: {
    name: string;
    conclusion: string | null;
    status: string;
    htmlUrl: string;
    updatedAt: string;
  } | null;
  framework?: string | null;
  /** Stack the scan detected, so remediation can prefer the matching catalog
   *  variant. Absent means default steps, never a guess. */
  detectedStack?: Record<string, unknown> | null;
  onDismiss?: (checkId: string) => void;
  onClose: () => void;
  onOpenCause?: (checkId: string) => void;
  onOpenIntegrations?: (integration: string) => void;
  onBack?: () => void;
  lifecycleInitialStatus?: WorkItemStatus | null;
  onLifecycleIgnore?: () => void | Promise<void>;
  onLifecycleBlock?: () => void | Promise<void>;
  onLifecycleReopen?: () => void | Promise<void>;
  onLifecycleResolved?: () => void | Promise<void>;
}

export function IssueDossier({
  selected,
  projectId,
  url,
  projectPath,
  latestScanId = null,
  onIssueLinkCreated,
  securityUpdates,
  nonSecurityUpdates,
  lastCIRun,
  framework,
  detectedStack = null,
  onDismiss,
  onClose,
  onOpenCause,
  onOpenIntegrations,
  onBack,
  lifecycleInitialStatus,
  onLifecycleIgnore,
  onLifecycleBlock,
  onLifecycleReopen,
  onLifecycleResolved,
}: IssueDossierProps) {
  const toast = useToast();
  const [verifyingCheckId, setVerifyingCheckId] = useState<string | null>(null);
  const selectedId = selected?.id ?? null;

  // Clear verification during render when the selected issue changes.
  useResetOnChange(selectedId, () => setVerifyingCheckId(null));

  const handleVerify = useCallback(
    async (checkId: string, sourceLabel: string) => {
      if (verifyingCheckId) return;
      setVerifyingCheckId(checkId);
      try {
        const outcome = await verifyIssue(projectId, normalizeAppUrlForKey(url), checkId);
        if (outcome.status === "verified") {
          await onLifecycleResolved?.();
          toast.success("Verified", `Fresh ${sourceLabel} evidence no longer detects this issue.`);
          onClose();
        } else if (outcome.status === "still_present") {
          toast.warning("Still present", `Fresh ${sourceLabel} evidence still detects this issue.`);
        } else {
          toast.success(
            "Verification started",
            "Source evidence is refreshing; the issue will update when verification finishes.",
          );
        }
      } catch (error) {
        toast.error(
          "Could not verify",
          userFacingError(error, "Run the verification again after the site has deployed."),
        );
      } finally {
        setVerifyingCheckId((current) => (current === checkId ? null : current));
      }
    },
    [onClose, onLifecycleResolved, projectId, toast, url, verifyingCheckId],
  );

  if (!selected) return null;

  if (selected.kind === "web") {
    return (
      <Suspense fallback={null}>
        <WebIssueDossier
          issue={selected.issue}
          group={selected.group}
          groupedIssues={selected.groupedIssues}
          projectId={projectId}
          url={url}
          projectPath={projectPath}
          latestScanId={latestScanId}
          estimatedImpact={selected.impact}
          detectedStack={detectedStack}
          onClose={onClose}
          onDismiss={onDismiss}
          onOpenCauseDossier={onOpenCause}
          onOpenIntegrations={onOpenIntegrations}
          onIssueLinkCreated={onIssueLinkCreated}
          onBack={onBack}
          lifecycleInitialStatus={lifecycleInitialStatus}
          onLifecycleIgnore={onLifecycleIgnore}
          onLifecycleBlock={onLifecycleBlock}
          onLifecycleReopen={onLifecycleReopen}
          onLifecycleResolved={onLifecycleResolved}
        />
      </Suspense>
    );
  }

  if (selected.kind === "code") {
    return (
      <Suspense fallback={null}>
        <CodeIssueDossier
          issue={selected.issue}
          group={selected.group}
          groupedIssues={selected.groupedIssues}
          projectId={projectId}
          scanUrl={url}
          projectPath={projectPath}
          framework={framework}
          onVerify={() =>
            void handleVerify(selected.group?.checkId ?? selected.issue.checkId, "Code Scan")
          }
          verifying={verifyingCheckId === (selected.group?.checkId ?? selected.issue.checkId)}
          onOpenIntegrations={onOpenIntegrations ? () => onOpenIntegrations("") : undefined}
          onClose={onClose}
          onBack={onBack}
          lifecycleInitialStatus={lifecycleInitialStatus}
          onLifecycleIgnore={onLifecycleIgnore}
          onLifecycleBlock={onLifecycleBlock}
          onLifecycleReopen={onLifecycleReopen}
        />
      </Suspense>
    );
  }

  if (selected.kind === "alert") {
    return (
      <IssueDossierPanel
        title={selected.issue.title}
        subtitle={selected.issue.description}
        eyebrow="Alert"
        onClose={onClose}
        onBack={onBack}>
        <Suspense fallback={null}>
          <AlertDetail
            alert={selected.issue}
            securityUpdates={securityUpdates}
            nonSecurityUpdates={nonSecurityUpdates}
            lastCIRun={lastCIRun}
          />
        </Suspense>
      </IssueDossierPanel>
    );
  }

  return null;
}
