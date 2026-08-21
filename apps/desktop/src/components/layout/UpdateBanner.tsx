import { useState, useEffect, useRef, useCallback } from "react";
import { checkAppUpdate } from "@/lib/commands";
import { onLatePrivilegedResolution } from "@/lib/privileged-command-bridge";
import { AlertTriangle, Download, RefreshCw, X } from "lucide-react";
import { ExtLink } from "@/components/ui/external-link";
import { Button } from "@/components/ui/button";
import { SUPPORT_EMAIL } from "@/lib/support";
import { useDesktopPrefs } from "@/lib/desktop-prefs";
import {
  installAppUpdate,
  relaunchApp,
  progressFraction,
  type UpdateCheckOutcome,
  type UpdateInstallOutcome,
  type AppUpdateProgress,
} from "@/lib/app-update";

const LEGACY_DISMISSED_KEY = "shk-dismissed-update";
const DISMISSED_KEY = "sitecmd-dismissed-update";
const SIGNATURE_DISMISSED_KEY = "sitecmd-dismissed-update-signature";
const DOWNLOAD_URL = "https://sitecmd.com/download";

// "silent" installs run in auto mode and render nothing until the restart pill;
// "manual" installs (the user clicked Install) render a progress banner.
type Phase = "idle" | "silent-installing" | "manual-installing" | "ready";

export function UpdateBanner() {
  const { prefs } = useDesktopPrefs();
  const [outcome, setOutcome] = useState<UpdateCheckOutcome | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [progress, setProgress] = useState<AppUpdateProgress | null>(null);
  const [installFailed, setInstallFailed] = useState(false);
  const [installTimedOut, setInstallTimedOut] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [readyDismissed, setReadyDismissed] = useState(false);
  const [signatureDismissed, setSignatureDismissed] = useState(
    () => sessionStorage.getItem(SIGNATURE_DISMISSED_KEY) === "1",
  );
  const autoStartedRef = useRef(false);
  const awaitingLateInstallRef = useRef(false);

  // Automatic installs await restart; manual installs relaunch immediately.
  const runInstall = useCallback(async (autoMode: boolean) => {
    awaitingLateInstallRef.current = false;
    setInstallFailed(false);
    setInstallTimedOut(false);
    setProgress(null);
    setPhase(autoMode ? "silent-installing" : "manual-installing");
    let result: UpdateInstallOutcome;
    try {
      result = await installAppUpdate(setProgress);
    } catch (error) {
      // A bridge timeout is inconclusive because the native installer may continue.
      if (error !== null && typeof error === "object" && "timeoutMs" in error) {
        awaitingLateInstallRef.current = true;
        setInstallTimedOut(true);
      } else {
        setInstallFailed(true);
      }
      setPhase("idle");
      return;
    }
    switch (result.kind) {
      case "installed":
        if (autoMode) {
          setPhase("ready");
        } else {
          try {
            await relaunchApp();
          } catch {
            // A restart failure does not undo a completed installation.
            setPhase("ready");
          }
        }
        break;
      case "signature_invalid":
        setOutcome({ kind: "signature_invalid", message: result.message });
        setPhase("idle");
        break;
      case "up_to_date":
      case "skipped":
        setOutcome({ kind: "up_to_date" });
        setPhase("idle");
        break;
      default:
        // network / unknown: fall back to the manual banner + website link.
        setInstallFailed(true);
        setPhase("idle");
    }
  }, []);

  // Launch check.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const result = await checkAppUpdate();
        if (!cancelled) setOutcome(result);
      } catch {
        // Pre-typed-outcome builds returned null on certain failures; tolerate
        // that quietly and let the next launch's updater try again.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Subscribe before an install starts so a result cannot land between timeout and render.
  useEffect(() => {
    return onLatePrivilegedResolution((late) => {
      if (late.command !== "download_and_install_app_update") return;
      if (!awaitingLateInstallRef.current) return;
      awaitingLateInstallRef.current = false;
      setInstallTimedOut(false);
      if (!late.ok) {
        setInstallFailed(true);
        return;
      }
      const value = late.value as { kind?: string; message?: string } | null;
      switch (value?.kind) {
        case "installed":
          setPhase("ready");
          break;
        case "signature_invalid":
          setOutcome({ kind: "signature_invalid", message: value.message ?? "" });
          break;
        case "up_to_date":
        case "skipped":
          setOutcome({ kind: "up_to_date" });
          break;
        default:
          setInstallFailed(true);
      }
    });
  }, []);

  // Auto mode: once an update is confirmed available, install it silently. The
  // ref guards against React StrictMode double-invoking the effect.
  useEffect(() => {
    if (autoStartedRef.current) return;
    if (outcome?.kind !== "available") return;
    if (!prefs.automaticUpdates) return;
    autoStartedRef.current = true;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- auto-starts the update install once, guarded by a ref; an imperative side effect
    void runInstall(true);
  }, [outcome, prefs.automaticUpdates, runInstall]);

  // Auto-mode install in flight: stay silent (no banner) until it resolves.
  if (phase === "silent-installing") return null;

  // Auto-mode install finished: offer a restart to apply it.
  if (phase === "ready" && !readyDismissed) {
    return (
      <div className="update-banner-panel" role="status">
        <RefreshCw className="icon-sm text-brand" />
        <span className="update-banner-text">
          <span className="update-banner-lead">Update installed.</span>
          <span className="update-banner-sub">Restart to finish.</span>
        </span>
        <Button
          unstyled
          type="button"
          onClick={() => void relaunchApp()}
          className="update-banner-action">
          Restart now
        </Button>
        <Button
          unstyled
          type="button"
          onClick={() => setReadyDismissed(true)}
          className="update-banner-dismiss"
          title="Dismiss; the update applies next time you restart SiteCMD">
          <X className="icon-sm" />
        </Button>
      </div>
    );
  }

  if (!outcome) return null;

  if (outcome.kind === "signature_invalid") {
    if (signatureDismissed) return null;
    return (
      <div className="update-banner-panel update-banner-panel--alert" role="alert">
        <AlertTriangle className="icon-sm" />
        <span className="update-banner-text">
          <strong>Update refused:</strong> the SiteCMD update server returned an unverifiable
          signature. We will not install this build. Download a fresh copy from sitecmd.com instead,
          or email {SUPPORT_EMAIL} if that does not help.
        </span>
        <ExtLink href={DOWNLOAD_URL} className="update-banner-action">
          Download
        </ExtLink>
        <Button
          unstyled
          type="button"
          onClick={() => {
            sessionStorage.setItem(SIGNATURE_DISMISSED_KEY, "1");
            setSignatureDismissed(true);
          }}
          className="update-banner-close"
          title="Dismiss for this session">
          <X className="icon-sm" />
        </Button>
      </div>
    );
  }

  // Manual install in flight: show download progress.
  if (phase === "manual-installing") {
    const fraction = progressFraction(progress);
    const label =
      fraction === null
        ? "Downloading update…"
        : `Downloading update… ${Math.round(fraction * 100)}%`;
    return (
      <div className="update-banner-panel" role="status" aria-live="polite">
        <Download className="icon-sm text-brand" />
        <span className="update-banner-text update-banner-lead">{label}</span>
      </div>
    );
  }

  if (outcome.kind !== "available") return null;
  if (dismissed) return null;

  // Don't re-show if the user already dismissed this version.
  const dismissedVersion =
    localStorage.getItem(DISMISSED_KEY) ?? localStorage.getItem(LEGACY_DISMISSED_KEY);
  if (dismissedVersion === outcome.version) return null;

  const handleDismiss = () => {
    setDismissed(true);
    localStorage.setItem(DISMISSED_KEY, outcome.version);
    localStorage.removeItem(LEGACY_DISMISSED_KEY);
  };

  return (
    <div className="update-banner-panel">
      <Download className="icon-sm text-brand" />
      <span className="update-banner-text">
        <span className="update-banner-lead">SiteCMD {outcome.version}</span>
        <span className="update-banner-sub">is available (you have {outcome.current_version})</span>
        {installFailed && (
          <span className="update-banner-sub">
            Automatic install failed; download it manually instead.
          </span>
        )}
        {installTimedOut && (
          <span className="update-banner-sub">
            The install is taking longer than expected and may still finish in the background.
            Restart SiteCMD later to check, or download it manually.
          </span>
        )}
      </span>
      {!installFailed && !installTimedOut && (
        <Button
          unstyled
          type="button"
          onClick={() => void runInstall(false)}
          className="update-banner-action">
          Install and restart
        </Button>
      )}
      <ExtLink href={DOWNLOAD_URL} className="update-banner-action">
        Download
      </ExtLink>
      <Button
        unstyled
        type="button"
        onClick={handleDismiss}
        className="update-banner-dismiss"
        title="Dismiss">
        <X className="icon-sm" />
      </Button>
    </div>
  );
}
