import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { arch, platform, version as osVersion } from "@tauri-apps/plugin-os";
import { Download, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useDesktopPrefs } from "@/lib/desktop-prefs";
import {
  checkAppUpdate,
  installAppUpdate,
  relaunchApp,
  progressFraction,
  type UpdateCheckOutcome,
  type UpdateInstallOutcome,
} from "@/lib/app-update";
import { queryKeys } from "@/lib/query/query-keys";
import { InlineSkeleton } from "@/components/ui/skeleton";

type CheckStatus =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up_to_date" }
  | { kind: "available"; version: string }
  | { kind: "installing"; percent: number | null }
  | { kind: "ready" }
  | { kind: "offline" }
  | { kind: "signature" }
  | { kind: "error" };

export function UpdatesSettingsCard() {
  const { prefs, updatePrefs } = useDesktopPrefs();
  const versionQuery = useQuery({
    queryKey: queryKeys.settings.appVersion(),
    queryFn: getVersion,
  });
  const version = versionQuery.data ?? null;
  const [status, setStatus] = useState<CheckStatus>({ kind: "idle" });
  // Development builds do not contact the updater service.
  const isDev = import.meta.env.DEV;

  const handleCheck = async () => {
    setStatus({ kind: "checking" });
    // Updater-construction failures must release the card's checking state.
    let outcome: UpdateCheckOutcome;
    try {
      outcome = await checkAppUpdate();
    } catch {
      setStatus({ kind: "error" });
      return;
    }
    switch (outcome.kind) {
      case "available":
        setStatus({ kind: "available", version: outcome.version });
        break;
      case "up_to_date":
        setStatus({ kind: "up_to_date" });
        break;
      case "network_unavailable":
        setStatus({ kind: "offline" });
        break;
      case "signature_invalid":
        setStatus({ kind: "signature" });
        break;
      default:
        setStatus({ kind: "error" });
    }
  };

  const handleInstall = async () => {
    setStatus({ kind: "installing", percent: null });
    // Clear a failed install's progress state.
    let result: UpdateInstallOutcome;
    try {
      result = await installAppUpdate((progress) => {
        const fraction = progressFraction(progress);
        setStatus({
          kind: "installing",
          percent: fraction === null ? null : Math.round(fraction * 100),
        });
      });
    } catch {
      setStatus({ kind: "error" });
      return;
    }
    switch (result.kind) {
      case "installed":
        setStatus({ kind: "ready" });
        await relaunchApp();
        break;
      case "up_to_date":
      case "skipped":
        setStatus({ kind: "up_to_date" });
        break;
      case "network_unavailable":
        setStatus({ kind: "offline" });
        break;
      case "signature_invalid":
        setStatus({ kind: "signature" });
        break;
      default:
        setStatus({ kind: "error" });
    }
  };

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Updates</h2>
      </div>
      <p className="body-muted settings-card-intro">
        Every update is signature-verified before it installs. You are on version{" "}
        <span className="font-mono">
          {versionQuery.isPending ? (
            <InlineSkeleton className="version-skeleton" />
          ) : version ? (
            `v${version}`
          ) : (
            "unknown"
          )}
        </span>
        <span className="font-mono">
          {" "}
          ({platform()} {osVersion()}, {arch()})
        </span>
        .
      </p>

      <div className="stack-tight">
        <div className="preference-toggle-row">
          <div className="icon-badge icon-badge--md icon-badge--primary">
            <RefreshCw className="preference-toggle-icon" />
          </div>
          <div className="flex-fill">
            <p className="row-title-md">Automatic updates</p>
            <p className="body-desc-xs">
              Download and install new versions in the background; they apply the next time you
              restart SiteCMD. Turn this off to check and install manually.
            </p>
          </div>
          <Button
            unstyled
            type="button"
            onClick={() => updatePrefs({ automaticUpdates: !prefs.automaticUpdates })}
            className="toggle-switch"
            data-on={prefs.automaticUpdates}
            aria-pressed={prefs.automaticUpdates}>
            <span className="toggle-switch-thumb" />
          </Button>
        </div>

        <div className="subtle-divider-top preference-toggle-row">
          <div className="flex-fill">
            <p className="row-title-md">Check for updates</p>
            <p className="body-desc-xs">{statusText(status, isDev)}</p>
          </div>
          {primaryAction(status, isDev, handleCheck, handleInstall)}
        </div>
      </div>
    </section>
  );
}

function statusText(status: CheckStatus, isDev: boolean): string {
  if (isDev) return "Updates are disabled in development builds.";
  switch (status.kind) {
    case "idle":
      return "Check whether a newer signed version is available.";
    case "checking":
      return "Checking for updates…";
    case "up_to_date":
      return "You are on the latest version.";
    case "available":
      return `Version ${status.version} is available.`;
    case "installing":
      return status.percent === null
        ? "Downloading update…"
        : `Downloading update… ${status.percent}%`;
    case "ready":
      return "Update installed. Restarting…";
    case "offline":
      return "Could not reach the update server. Check your connection and try again.";
    case "signature":
      // Names the way out, like the banner does. A signature this build cannot
      // verify is not something checking again will fix.
      return "Update refused: the server returned an unverifiable signature. Nothing was installed. Download a fresh copy from sitecmd.com to continue receiving updates.";
    case "error":
      return "Something went wrong checking for updates. Try again.";
  }
}

function primaryAction(
  status: CheckStatus,
  isDev: boolean,
  onCheck: () => void,
  onInstall: () => void,
) {
  if (isDev) {
    return (
      <Button variant="outline" disabled>
        <RefreshCw className="icon-md" />
        Check for updates
      </Button>
    );
  }
  if (status.kind === "available") {
    return (
      <Button onClick={() => void onInstall()}>
        <Download className="icon-md" />
        Install and restart
      </Button>
    );
  }
  const busy =
    status.kind === "checking" || status.kind === "installing" || status.kind === "ready";
  return (
    <Button variant="outline" onClick={() => void onCheck()} disabled={busy}>
      <RefreshCw className="icon-md" />
      Check for updates
    </Button>
  );
}
