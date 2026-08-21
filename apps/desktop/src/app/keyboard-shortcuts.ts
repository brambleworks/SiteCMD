import type { NavPage } from "@/components/layout/NavSidebar";

export interface KeyBinding {
  key: string;
  label: string;
}

/** Cmd/Ctrl + key to jump to a page. Number order drives the ⌘1..⌘7 sequence. */
export const PAGE_SHORTCUTS: Partial<Record<NavPage, KeyBinding>> = {
  dashboard: { key: "1", label: "⌘1" },
  events: { key: "2", label: "⌘2" },
  analytics: { key: "3", label: "⌘3" },
  "search-console": { key: "4", label: "⌘4" },
  issues: { key: "5", label: "⌘5" },
  deploys: { key: "6", label: "⌘6" },
  updates: { key: "7", label: "⌘7" },
  settings: { key: ",", label: "⌘," },
};

/** Cmd/Ctrl + key for app actions that are not page navigation. */
export const ACTION_SHORTCUTS: Record<"commandPalette" | "addProject" | "runScan", KeyBinding> = {
  commandPalette: { key: "k", label: "⌘K" },
  addProject: { key: "n", label: "⌘N" },
  runScan: { key: "r", label: "⌘R" },
};
