import { useSyncExternalStore } from "react";
import { storeSet, migrateFromLocalStorage } from "@/lib/store";
import { isJsonRecord } from "@/lib/json-record";

interface DesktopPreferences {
  backgroundMonitoring: boolean;
  fileWatchSuggestions: boolean;
  desktopNotifications: boolean;
  refreshOnFocus: boolean;
  // Automatic updates apply on restart; manual mode waits for user approval.
  automaticUpdates: boolean;
}

const STORAGE_KEY = "sitecmd-desktop-prefs";
const STORE_KEY = "desktop-prefs";

const DEFAULTS: DesktopPreferences = {
  backgroundMonitoring: false,
  fileWatchSuggestions: false,
  desktopNotifications: true,
  refreshOnFocus: true,
  automaticUpdates: true,
};

let prefs = loadPreferences();
const listeners = new Set<() => void>();

// Async hydration from Tauri store on module load
migrateFromLocalStorage<DesktopPreferences>(STORAGE_KEY, STORE_KEY, DEFAULTS, parsePreferences)
  .then((stored) => {
    prefs = { ...DEFAULTS, ...stored };
    for (const listener of listeners) listener();
  })
  .catch(() => {});

function loadPreferences(): DesktopPreferences {
  if (typeof window === "undefined") return DEFAULTS;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULTS;
    return parsePreferences(JSON.parse(raw) as unknown) ?? DEFAULTS;
  } catch {
    return DEFAULTS;
  }
}

function parsePreferences(value: unknown): DesktopPreferences | null {
  if (!isJsonRecord(value)) return null;
  return {
    backgroundMonitoring:
      typeof value.backgroundMonitoring === "boolean"
        ? value.backgroundMonitoring
        : DEFAULTS.backgroundMonitoring,
    fileWatchSuggestions:
      typeof value.fileWatchSuggestions === "boolean"
        ? value.fileWatchSuggestions
        : DEFAULTS.fileWatchSuggestions,
    desktopNotifications:
      typeof value.desktopNotifications === "boolean"
        ? value.desktopNotifications
        : DEFAULTS.desktopNotifications,
    refreshOnFocus:
      typeof value.refreshOnFocus === "boolean" ? value.refreshOnFocus : DEFAULTS.refreshOnFocus,
    automaticUpdates:
      typeof value.automaticUpdates === "boolean"
        ? value.automaticUpdates
        : DEFAULTS.automaticUpdates,
  };
}

function persist() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // best effort
  }
  storeSet(STORE_KEY, prefs).catch(() => {});
}

function publish() {
  for (const listener of listeners) listener();
}

export function setDesktopPrefs(next: DesktopPreferences) {
  prefs = {
    ...DEFAULTS,
    ...next,
  };
  persist();
  publish();
}

export function updateDesktopPrefs(patch: Partial<DesktopPreferences>) {
  setDesktopPrefs({
    ...prefs,
    ...patch,
  });
}

function subscribe(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

function getSnapshot() {
  return prefs;
}

export function useDesktopPrefs() {
  const current = useSyncExternalStore(subscribe, getSnapshot);
  return {
    prefs: current,
    setPrefs: setDesktopPrefs,
    updatePrefs: updateDesktopPrefs,
  };
}
