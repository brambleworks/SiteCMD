import type { AppTarget } from "@/lib/app-targets";
import { openPathInEditor } from "@/lib/desktop-actions";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { handleDesktopNotificationAction } from "@/app/app-shell-helpers";

export function useDesktopNotificationActions(openAppTarget: (target: AppTarget) => void) {
  // Route notification actions best-effort; navigation failures are contained.
  useTauriEvent("desktop-notification-action", (payload) => {
    void handleDesktopNotificationAction(payload, {
      openFilePath: (path) => openPathInEditor(path).catch(() => {}),
      openTarget: openAppTarget,
    });
  });
}
