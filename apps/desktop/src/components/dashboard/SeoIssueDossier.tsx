import { useMemo } from "react";
import { copyToClipboard } from "@/lib/clipboard";
import { coerceJsonRecord } from "@/lib/json-record";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { Button } from "@/components/ui/button";
import { Markdown } from "@/components/ui/markdown";
import { IssueActionBar } from "@/components/issues/IssueActionBar";
import { FixWithAgentAction } from "@/components/issues/FixWithAgentAction";
import { useFixAttempt } from "@/components/issues/useFixAttempt";
import { useToast } from "@/hooks/useToast";
import {
  DossierKeyValueGrid,
  DossierRail,
  IssueDossierPanel,
} from "@/components/issues/IssueDossierPanel";
import { formatSeverityLabel as toSeverityLabel, formatSeverityToneClass } from "@/lib/severity";
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
import { EnrichmentSection } from "@/components/issues/EnrichmentSection";
import { CommandExecutionPanel } from "@/components/issues/CommandExecutionPanel";
import { IssueMemoryRail } from "@/components/issues/IssueMemorySection";
import { DossierRecentChangeRail } from "@/components/issues/DossierRailDetails";
import { AsyncFixGuideSteps } from "@/components/ui/AsyncFixGuideSteps";
import { CATEGORY_LABELS } from "@/lib/tokens";
import { extractDesktopCommands } from "@/lib/desktop-actions";
import {
  getLatestDesktopPrompt,
  useDesktopPromptCenter,
  type DesktopPromptEntry,
} from "@/lib/desktop-prompts";
import { getCheckIssueScope } from "@/lib/issue-scope";
import type { CheckResult, IssueGroup } from "@/lib/types";
import { buildPendingVerificationId, resolvePendingVerification } from "@/lib/pending-verification";
import { useIssueDossierActions } from "@/components/issues/useIssueDossierActions";
import { getCopyActionLabel } from "@/lib/action-language";
import { formatUrlPathOrHost } from "@/lib/utils";
import { inferSeoFocus } from "@/components/dashboard/search-console-page-model";

export function SeoIssueDossier({
  issue,
  detectedStack = null,
  group,
  projectId,
  url,
  projectPath,
  arrivalPrompt,
  onClose,
  onVerify,
  verifying,
  onOpenScan,
}: {
  issue: CheckResult;
  /** Stack the scan detected, forwarded to guide resolution so the catalog's
   *  matching variant is preferred over the generic default steps. */
  detectedStack?: Record<string, unknown> | null;
  group?: IssueGroup;
  projectId: number;
  url: string;
  projectPath?: string;
  arrivalPrompt?: DesktopPromptEntry | null;
  onClose: () => void;
  onVerify: () => void;
  verifying: boolean;
  onOpenScan: () => void;
}) {
  const toast = useToast();
  const desktopPrompts = useDesktopPromptCenter();
  const fixText = issue.manualFix || "";
  const commands = extractDesktopCommands(fixText);
  const normalizedUrl = useMemo(() => normalizeAppUrlForKey(url), [url]);
  const { attempt, setAttempt } = useFixAttempt({
    projectId,
    envUrl: normalizedUrl,
    checkId: issue.checkId,
    title: issue.title,
  });
  const likelyChangedPrompt = useMemo(() => {
    if (
      arrivalPrompt &&
      arrivalPrompt.projectId === projectId &&
      arrivalPrompt.page === "search-console" &&
      normalizeAppUrlForKey(arrivalPrompt.url) === normalizedUrl
    ) {
      return arrivalPrompt;
    }
    return getLatestDesktopPrompt(desktopPrompts, {
      projectId,
      url,
      page: "search-console",
    });
  }, [arrivalPrompt, desktopPrompts, normalizedUrl, projectId, url]);
  const scopeMeta = getCheckIssueScope(issue, url);
  const categoryLabel = CATEGORY_LABELS[issue.category] ?? issue.category;
  const affectedPath = useMemo(() => formatUrlPathOrHost(url, url), [url]);
  const seoFocus = useMemo(() => inferSeoFocus(issue), [issue]);

  const {
    correlatedFiles,
    primaryCorrelatedFile,
    queueWorkingState,
    runFirstCommand,
    runningCommand,
    lastCommandResult,
    openEditor: handleOpenEditor,
    openFile,
    revealTarget: handleRevealFolder,
    revealFile,
  } = useIssueDossierActions({
    issue,
    projectId,
    url,
    projectPath,
    page: "search-console",
    focus: seoFocus,
    preferredLocation: likelyChangedPrompt,
    reasons: {
      openedPath: "Opened SEO fix file",
      revealedPath: "Revealed SEO fix file",
      ranCommand: "Ran SEO fix command",
    },
  });

  const handleCopyPrompt = async () => {
    if (!fixText) return;
    // Blocked clipboard writes resolve to false.
    const copied = await copyToClipboard(fixText);
    if (copied) {
      await queueWorkingState("Copied SEO fix guidance");
    } else {
      toast.error(
        "Couldn't copy",
        "Clipboard access was blocked. Try again, or copy the guidance manually.",
      );
    }
  };

  const handleRunFirstCommand = () => runFirstCommand(commands);

  // Opening a fix location starts the same workflow as other dossiers.
  const handleOpenFile = (file: IssueAffectedFile) => {
    const original = correlatedFiles.find((f) => f.absolutePath === file.key);
    if (!original) return;
    void openFile(original);
  };

  const handleRevealFile = (file: IssueAffectedFile) => {
    const original = correlatedFiles.find((f) => f.absolutePath === file.key);
    if (!original) return;
    void revealFile(original);
  };

  const affectedFiles: IssueAffectedFile[] = correlatedFiles.map((file) => ({
    key: file.absolutePath,
    label: file.label,
    reason: file.reason,
    relativePath: file.relativePath,
  }));

  const leftRail = (
    <>
      {likelyChangedPrompt ? <DossierRecentChangeRail prompt={likelyChangedPrompt} /> : null}

      <DossierRail className="dossier-rail-section-plain">
        <div className="dossier-rail-list">
          <div className="dossier-rail-row">
            <span className="dossier-rail-row-key">Applies to</span>
            <span className="dossier-rail-row-value">{scopeMeta.scopeLabel}</span>
          </div>
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
              url={url}
              whyItMatters={issue.whyItMatters}
              evidence={issue.rawData}
              manualFix={issue.manualFix}
              previousFailure={attempt?.status === "verify_failed" ? attempt.failureDetail : null}
              projectPath={projectPath}
              onAttemptCreated={setAttempt}
            />
          }
          onIgnore={() => {
            resolvePendingVerification(
              buildPendingVerificationId(projectId, normalizedUrl, issue.checkId, "search-console"),
            );
          }}
          onBlock={() => {
            resolvePendingVerification(
              buildPendingVerificationId(projectId, normalizedUrl, issue.checkId, "search-console"),
            );
          }}
        />
      </DossierRail>
    </>
  );

  const hasRawEvidence = Boolean(issue.rawData && Object.keys(issue.rawData).length > 0);
  const enrichments = group?.enrichments ?? [];

  return (
    <IssueDossierPanel
      title={issue.title}
      subtitle={issue.whyItMatters ?? undefined}
      eyebrow={
        <>
          <span className={formatSeverityToneClass(issue.severity)}>
            {toSeverityLabel(issue.severity)}
          </span>
          {` - ${categoryLabel}`}
        </>
      }
      leftRail={leftRail}
      rightRail={rightRail}
      onClose={onClose}>
      <IssueWhatSection description={issue.description} />

      {group ? <IssueV3HeaderExtras issue={group} /> : null}

      <IssueWhereLivesSection
        pages={[{ key: url, label: affectedPath }]}
        files={affectedFiles}
        onOpenFile={handleOpenFile}
        onRevealFile={handleRevealFile}
      />

      <IssueHowToFixSection>
        <div className="row-loose">
          {projectPath && commands.length > 0 ? (
            <Button unstyled type="button" onClick={handleRunFirstCommand} className="link-success">
              Run first command
            </Button>
          ) : null}
          {fixText ? (
            <Button unstyled type="button" onClick={handleCopyPrompt} className="link-muted">
              {getCopyActionLabel("fix-steps")}
            </Button>
          ) : null}
        </div>
        <AsyncFixGuideSteps
          kind="web"
          checkId={issue.checkId}
          detectedStack={detectedStack}
          fallback={
            fixText ? (
              <div className="card-sunken">
                <Markdown>{fixText}</Markdown>
              </div>
            ) : (
              <p className="body-text-muted">
                This issue does not have attached fix steps yet. Use the evidence above, make the
                change, then verify the exact issue again from this panel.
              </p>
            )
          }
        />
        <div className="row-wrap">
          {projectPath ? (
            <Button
              variant="ghost"
              className="dossier-open-file-btn text-meta"
              onClick={handleOpenEditor}>
              {likelyChangedPrompt?.absolutePath
                ? "Open changed file"
                : primaryCorrelatedFile
                  ? "Open likely file"
                  : "Open in editor"}
            </Button>
          ) : null}
          {projectPath ? (
            <Button
              variant="ghost"
              className="dossier-open-file-btn text-meta"
              onClick={handleRevealFolder}>
              Reveal folder
            </Button>
          ) : null}
          <Button variant="ghost" className="dossier-open-file-btn text-meta" onClick={onOpenScan}>
            Open Web Scan
          </Button>
        </div>
        {commands.length > 0 ? (
          <CommandExecutionPanel
            command={commands[0]!}
            result={lastCommandResult}
            running={runningCommand}
            onVerify={onVerify}
            verifying={verifying}
          />
        ) : null}
      </IssueHowToFixSection>

      {enrichments.length > 0 ? <EnrichmentSection enrichments={enrichments} /> : null}

      {hasRawEvidence ? (
        <IssueProofSection summary="Observed Search & SEO evidence.">
          <ProofBlock>
            <DossierKeyValueGrid data={coerceJsonRecord(issue.rawData) ?? {}} />
          </ProofBlock>
        </IssueProofSection>
      ) : null}

      {group ? <IssueV3Footer issue={group} /> : null}
    </IssueDossierPanel>
  );
}
