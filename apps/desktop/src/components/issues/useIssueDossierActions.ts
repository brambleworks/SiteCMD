import { useCallback, useEffect, useMemo, useState } from "react";
import { resolveFixLocationsForCheck } from "@/lib/commands";
import { normalizeAppUrlForKey, type AppTargetPage } from "@/lib/app-targets";
import type { CheckResult, FixLocation } from "@/lib/types";
import {
  isProjectCommandCancelled,
  openPathInEditor,
  revealPath,
  runProjectCommand,
  type DesktopCommandResult,
} from "@/lib/desktop-actions";
import { queuePendingVerification } from "@/lib/pending-verification";
import { useToast } from "@/hooks/useToast";
import { userFacingError } from "@/lib/user-facing-error";

/** Minimal file shape the open/reveal handlers need; FixLocation satisfies it. */
interface DossierFileTarget {
  absolutePath: string;
  relativePath?: string | null;
}

export interface IssueDossierActionsConfig {
  issue: CheckResult;
  projectId: number | null | undefined;
  url: string;
  projectPath: string | null | undefined;
  /** Deep-link target page used for queued verification. */
  page: AppTargetPage;
  focus: string | null;
  /** Preferred fix location before correlated-file fallbacks. */
  preferredLocation?: { absolutePath?: string | null; relativePath?: string | null } | null;
  /** Verification reasons queued when the user acts from this dossier. */
  reasons: {
    openedPath: string;
    revealedPath: string;
    ranCommand?: string;
  };
}

export interface IssueDossierActions {
  correlatedFiles: FixLocation[];
  primaryCorrelatedFile: FixLocation | null;
  queueWorkingState: (reason: string, filePath?: string | null) => Promise<void>;
  runFirstCommand: (commands: string[]) => Promise<void>;
  runningCommand: boolean;
  lastCommandResult: DesktopCommandResult | null;
  openEditor: () => Promise<void>;
  openFile: (file: DossierFileTarget) => Promise<void>;
  revealTarget: () => Promise<void>;
  revealFile: (file: DossierFileTarget) => Promise<void>;
}

export function getIssuePageTarget(issue: CheckResult): {
  page: AppTargetPage;
  focus: string | null;
} {
  if (issue.category === "seo") return { page: "search-console", focus: null };
  return { page: "issues", focus: issue.category };
}

/** Queue verification after successful dossier fix actions. */
export function useIssueDossierActions(config: IssueDossierActionsConfig): IssueDossierActions {
  const { issue, projectId, url, projectPath, page, focus, preferredLocation, reasons } = config;
  const toast = useToast();
  const [correlatedFiles, setCorrelatedFiles] = useState<FixLocation[]>([]);
  const [runningCommand, setRunningCommand] = useState(false);
  const [lastCommandResult, setLastCommandResult] = useState<DesktopCommandResult | null>(null);
  const normalizedUrl = useMemo(() => normalizeAppUrlForKey(url), [url]);
  const primaryCorrelatedFile = correlatedFiles[0] ?? null;

  useEffect(() => {
    let cancelled = false;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- clears correlated files before the async fix-location resolve for the new check
    setCorrelatedFiles([]);
    if (!projectPath || projectId == null) return;

    resolveFixLocationsForCheck({
      checkId: issue.checkId,
      projectId,
    })
      .then((files) => {
        if (!cancelled) setCorrelatedFiles(files ?? []);
      })
      .catch(() => {
        if (!cancelled) setCorrelatedFiles([]);
      });

    return () => {
      cancelled = true;
    };
  }, [issue.checkId, projectId, projectPath]);

  const defaultFilePath =
    preferredLocation?.absolutePath || primaryCorrelatedFile?.absolutePath || projectPath || null;
  const defaultRelativePath =
    preferredLocation?.relativePath ?? primaryCorrelatedFile?.relativePath ?? undefined;

  const queueWorkingState = useCallback(
    async (reason: string, filePath?: string | null) => {
      if (!projectId) return;
      const nextFilePath = filePath ?? defaultFilePath;
      queuePendingVerification({
        projectId,
        url: normalizedUrl,
        itemId: issue.checkId,
        label: issue.title,
        reason,
        page,
        focus,
        filePath: nextFilePath,
      });
    },
    [defaultFilePath, focus, issue, normalizedUrl, page, projectId],
  );

  const runFirstCommand = async (commands: string[]) => {
    if (!projectPath || commands.length === 0) return;
    try {
      setRunningCommand(true);
      const result = await runProjectCommand(projectPath, commands[0]!);
      setLastCommandResult(result);
      if (result.success) {
        // Verification is only worth queueing when the command actually ran.
        await queueWorkingState(reasons.ranCommand ?? "Ran fix command");
        toast.success("Command finished", result.stdout || "The command completed successfully.");
      } else {
        toast.error(
          "Command failed",
          result.stderr || result.stdout || "The command exited with an error.",
        );
      }
    } catch (err) {
      if (isProjectCommandCancelled(err)) return;
      setLastCommandResult(null);
      toast.error(
        "Command failed",
        userFacingError(err, "Check that the path still exists and SiteCMD can read it."),
      );
    } finally {
      setRunningCommand(false);
    }
  };

  const openEditor = async () => {
    if (!defaultFilePath) return;
    try {
      await openPathInEditor(defaultFilePath);
      await queueWorkingState(reasons.openedPath, defaultFilePath);
      toast.success("Opened in editor", defaultRelativePath);
    } catch (err) {
      toast.error(
        "Could not open editor",
        userFacingError(
          err,
          "SiteCMD could not open your editor. Open the file yourself and paste the prompt.",
        ),
      );
    }
  };

  const openFile = async (file: DossierFileTarget) => {
    try {
      await openPathInEditor(file.absolutePath);
      await queueWorkingState(reasons.openedPath, file.absolutePath);
      toast.success("Opened in editor", file.relativePath ?? undefined);
    } catch (err) {
      toast.error(
        "Could not open editor",
        userFacingError(
          err,
          "SiteCMD could not open your editor. Open the file yourself and paste the prompt.",
        ),
      );
    }
  };

  const revealTarget = async () => {
    if (!defaultFilePath) return;
    const isProjectRoot = defaultFilePath === projectPath;
    try {
      await revealPath(defaultFilePath);
      await queueWorkingState(reasons.revealedPath, defaultFilePath);
      if (isProjectRoot) {
        toast.success("Revealed project folder");
      } else {
        toast.success("Revealed file", defaultRelativePath);
      }
    } catch (err) {
      toast.error(
        isProjectRoot ? "Could not reveal folder" : "Could not reveal file",
        userFacingError(err, "SiteCMD could not open it. Open the file from your editor instead."),
      );
    }
  };

  const revealFile = async (file: DossierFileTarget) => {
    try {
      await revealPath(file.absolutePath);
      await queueWorkingState(reasons.revealedPath, file.absolutePath);
      toast.success("Revealed file", file.relativePath ?? undefined);
    } catch (err) {
      toast.error(
        "Could not reveal file",
        userFacingError(err, "SiteCMD could not open it. Open the file from your editor instead."),
      );
    }
  };

  return {
    correlatedFiles,
    primaryCorrelatedFile,
    queueWorkingState,
    runFirstCommand,
    runningCommand,
    lastCommandResult,
    openEditor,
    openFile,
    revealTarget,
    revealFile,
  };
}
