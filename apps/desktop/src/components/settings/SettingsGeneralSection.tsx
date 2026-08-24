import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { UpdatesSettingsCard } from "./UpdatesSettingsCard";
import { useToast } from "@/hooks/useToast";
import { useDesktopPrefs } from "@/lib/desktop-prefs";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { Bell, FolderSync, Monitor, PlayCircle, RefreshCw } from "lucide-react";
import { queryKeys } from "@/lib/query/query-keys";
import { InlineSkeleton } from "@/components/ui/skeleton";
import { userFacingError } from "@/lib/user-facing-error";

export function GeneralSection() {
  const { prefs: desktopPrefs, updatePrefs: updateDesktopPrefs } = useDesktopPrefs();
  const queryClient = useQueryClient();
  const autostartQueryKey = queryKeys.settings.autostart();
  const autostartQuery = useQuery({
    queryKey: autostartQueryKey,
    queryFn: isAutostartEnabled,
  });
  const launchAtLogin = autostartQuery.data ?? false;
  const loadingLaunchAtLogin = autostartQuery.isPending;
  const [savingLaunchAtLogin, setSavingLaunchAtLogin] = useState(false);
  const toast = useToast();

  const toggleLaunchAtLogin = async () => {
    if (loadingLaunchAtLogin || savingLaunchAtLogin) return;
    setSavingLaunchAtLogin(true);
    try {
      if (launchAtLogin) {
        await disableAutostart();
        queryClient.setQueryData(autostartQueryKey, false);
        toast.success("Launch at login disabled");
      } else {
        await enableAutostart();
        queryClient.setQueryData(autostartQueryKey, true);
        toast.success("Launch at login enabled");
      }
    } catch (error) {
      toast.error(
        "Could not update launch at login",
        userFacingError(error, "Your change was not saved. Try again."),
      );
    } finally {
      setSavingLaunchAtLogin(false);
    }
  };

  return (
    <div className="settings-section-stack">
      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Background Behavior</h2>
        </div>
        <p className="body-muted settings-card-intro">
          Decide when SiteCMD should keep watching for useful signals while you work in other apps.
        </p>
        <div className="stack-tight">
          <PreferenceToggle
            icon={PlayCircle}
            label="Launch at login"
            description="Open SiteCMD automatically when you sign in."
            enabled={launchAtLogin}
            loading={loadingLaunchAtLogin}
            error={autostartQuery.isError}
            disabled={loadingLaunchAtLogin || savingLaunchAtLogin}
            onToggle={toggleLaunchAtLogin}
            onRetry={() => void autostartQuery.refetch()}
          />
          <PreferenceToggle
            icon={Monitor}
            label="Keep monitors running"
            description="Allow lightweight local checks while SiteCMD is open, even when the window is not frontmost."
            enabled={desktopPrefs.backgroundMonitoring}
            onToggle={() =>
              updateDesktopPrefs({ backgroundMonitoring: !desktopPrefs.backgroundMonitoring })
            }
          />
          <PreferenceToggle
            icon={FolderSync}
            label="Suggest re-checks after file changes"
            description="Watch package files, robots.txt, sitemaps, headers, and config files so SiteCMD can point you to the right follow-up scan."
            enabled={desktopPrefs.fileWatchSuggestions}
            disabled={!desktopPrefs.backgroundMonitoring}
            onToggle={() =>
              updateDesktopPrefs({ fileWatchSuggestions: !desktopPrefs.fileWatchSuggestions })
            }
          />
          <PreferenceToggle
            icon={Bell}
            label="Desktop notifications"
            description="Show OS notifications for background scan results and important follow-up prompts."
            enabled={desktopPrefs.desktopNotifications}
            onToggle={() =>
              updateDesktopPrefs({ desktopNotifications: !desktopPrefs.desktopNotifications })
            }
          />
          <PreferenceToggle
            icon={RefreshCw}
            label="Refresh when you return"
            description="Re-check local monitors when you bring SiteCMD back to the front."
            enabled={desktopPrefs.refreshOnFocus}
            disabled={!desktopPrefs.backgroundMonitoring}
            onToggle={() => updateDesktopPrefs({ refreshOnFocus: !desktopPrefs.refreshOnFocus })}
          />
        </div>
      </section>

      <UpdatesSettingsCard />
    </div>
  );
}

function PreferenceToggle({
  icon: Icon,
  label,
  description,
  enabled,
  onToggle,
  disabled = false,
  loading = false,
  error = false,
  onRetry,
}: {
  icon: typeof Monitor;
  label: string;
  description: string;
  enabled: boolean;
  onToggle: () => void;
  disabled?: boolean;
  loading?: boolean;
  error?: boolean;
  onRetry?: () => void;
}) {
  return (
    <div className="subtle-divider-top preference-toggle-row">
      <div className="icon-badge icon-badge--md icon-badge--primary">
        <Icon className="preference-toggle-icon" />
      </div>
      <div className="flex-fill">
        <p className="row-title-md">{label}</p>
        <p className="body-desc-xs">{description}</p>
      </div>
      {error ? (
        <Button variant="outline" size="sm" onClick={onRetry}>
          Retry
        </Button>
      ) : loading ? (
        <InlineSkeleton className="toggle-switch-skeleton" />
      ) : (
        <Button
          unstyled
          type="button"
          onClick={onToggle}
          disabled={disabled}
          className="toggle-switch"
          data-on={enabled}
          aria-pressed={enabled}>
          <span className="toggle-switch-thumb" />
        </Button>
      )}
    </div>
  );
}
