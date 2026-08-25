import { useEffect } from "react";
import type { NavPage } from "@/components/layout/NavSidebar";
import { toNavPage } from "@/components/layout/nav-page";
import { ACTION_SHORTCUTS, PAGE_SHORTCUTS } from "@/app/keyboard-shortcuts";

/** Some Dialog surfaces demand a choice before the app moves on (the telemetry
 *  consent prompt cannot be dismissed at all), so no shortcut may navigate away
 *  or stack a second top-layer surface while any modal dialog is open. */
function modalDialogIsOpen(): boolean {
  return document.querySelector("dialog[open]") !== null;
}

export function useAppKeyboardShortcuts({
  activeEnvUrl,
  enabledCategories,
  navigateTo,
  openAddProject,
  openCommandPalette,
  openScanConfig,
  page,
  scan,
  scanState,
  timeout,
}: {
  activeEnvUrl: string | null;
  enabledCategories: string[];
  navigateTo: (target: string) => void;
  openAddProject: () => void;
  openCommandPalette: () => void;
  openScanConfig: () => void;
  page: NavPage;
  scan: (
    url: string,
    options?: {
      enabledCategories?: string[];
      timeoutSecs?: number;
    },
  ) => unknown;
  scanState: string;
  timeout: number;
}) {
  useEffect(() => {
    const pageByKey = new Map<string, NavPage>();
    for (const [navPage, binding] of Object.entries(PAGE_SHORTCUTS)) {
      if (binding) pageByKey.set(binding.key, toNavPage(navPage));
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (modalDialogIsOpen()) return;
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      if (e.key === ACTION_SHORTCUTS.commandPalette.key) {
        e.preventDefault();
        openCommandPalette();
        return;
      }
      const targetPage = pageByKey.get(e.key);
      if (targetPage) {
        e.preventDefault();
        navigateTo(targetPage);
        return;
      }
      if (e.key === ACTION_SHORTCUTS.addProject.key) {
        e.preventDefault();
        openAddProject();
        return;
      }
      if (
        e.key === ACTION_SHORTCUTS.runScan.key &&
        activeEnvUrl &&
        (page === "dashboard" || page === "issues")
      ) {
        e.preventDefault();
        if (scanState === "idle") {
          void scan(activeEnvUrl, { enabledCategories, timeoutSecs: timeout });
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    page,
    activeEnvUrl,
    scanState,
    scan,
    enabledCategories,
    timeout,
    navigateTo,
    openAddProject,
    openCommandPalette,
  ]);

  useEffect(() => {
    let cleanup: (() => void) | null = null;
    import("@tauri-apps/plugin-global-shortcut")
      .then(({ register, unregisterAll }) => {
        const registered: string[] = [];
        register("CmdOrCtrl+Shift+S", () => {
          import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
            const w = getCurrentWindow();
            w.unminimize();
            w.show();
            w.setFocus();
          });
          // Still summon the window, but never stack scan config on top of an
          // open modal dialog; this shortcut bypasses the webview's inertness.
          if (!modalDialogIsOpen()) openScanConfig();
        })
          .then(() => registered.push("CmdOrCtrl+Shift+S"))
          .catch(() => {});
        register("CmdOrCtrl+Shift+H", () => {
          import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
            const w = getCurrentWindow();
            w.unminimize();
            w.show();
            w.setFocus();
          });
        })
          .then(() => registered.push("CmdOrCtrl+Shift+H"))
          .catch(() => {});

        cleanup = () => {
          unregisterAll().catch(() => {});
        };
      })
      .catch(() => {});
    return () => {
      cleanup?.();
    };
  }, [openScanConfig]);
}
