import { useState, useEffect, useCallback, useMemo, createContext, useContext } from "react";
import {
  migrateLegacyValue,
  readCurrentOrLegacyValue,
  writeCurrentValue,
} from "@/lib/local-storage-migration";
import { isJsonRecord } from "@/lib/json-record";

export interface ScanPreferences {
  timeout: number; // seconds
  retentionLimit: number; // max scans to keep per site (default 50)
  categories: {
    security: boolean;
    performance: boolean;
    seo: boolean;
    accessibility: boolean;
    compliance: boolean;
    config: boolean;
  };
}

const DEFAULTS: ScanPreferences = {
  timeout: 30,
  retentionLimit: 50,
  categories: {
    security: true,
    performance: true,
    seo: true,
    accessibility: true,
    compliance: true,
    config: true,
  },
};

const STORAGE_KEY = "sitecmd-scan-prefs";
const LEGACY_STORAGE_KEY = "sitehealthkit-scan-prefs";
const STORE_KEY = "scan-prefs";
const TIMEOUT_MIN = 10;
const TIMEOUT_MAX = 60;
const RETENTION_MIN = 5;
const RETENTION_MAX = 100;

interface ScanPrefsContextValue {
  prefs: ScanPreferences;
  setPrefs: (p: ScanPreferences) => void;
  enabledCategories: string[];
}

const ScanPrefsContext = createContext<ScanPrefsContextValue>({
  prefs: DEFAULTS,
  setPrefs: () => {},
  enabledCategories: Object.keys(DEFAULTS.categories),
});

export function useScanPrefs() {
  return useContext(ScanPrefsContext);
}

function load(): ScanPreferences {
  try {
    const stored = readCurrentOrLegacyValue(localStorage, STORAGE_KEY, LEGACY_STORAGE_KEY);
    if (stored) {
      const parsed = parseScanPreferences(JSON.parse(stored) as unknown);
      if (parsed) return parsed;
    }
  } catch {
    // Corrupt localStorage - fall through to defaults
  }
  return DEFAULTS;
}

function boundedInteger(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, Math.round(value)));
}

export function parseScanPreferences(value: unknown): ScanPreferences | null {
  if (!isJsonRecord(value)) return null;
  return {
    timeout: boundedInteger(value.timeout, TIMEOUT_MIN, TIMEOUT_MAX, DEFAULTS.timeout),
    retentionLimit: boundedInteger(
      value.retentionLimit,
      RETENTION_MIN,
      RETENTION_MAX,
      DEFAULTS.retentionLimit,
    ),
    categories: DEFAULTS.categories,
  };
}

export function ScanPrefsProvider({ children }: { children: React.ReactNode }) {
  const [prefs, setPrefsState] = useState<ScanPreferences>(load);

  const setPrefs = useCallback((p: ScanPreferences) => {
    const normalized = parseScanPreferences(p) ?? DEFAULTS;
    setPrefsState(normalized);
    writeCurrentValue(localStorage, STORAGE_KEY, LEGACY_STORAGE_KEY, JSON.stringify(normalized));
    import("@/lib/store").then(({ storeSet }) => storeSet(STORE_KEY, normalized)).catch(() => {});
  }, []);

  useEffect(() => {
    migrateLegacyValue(localStorage, STORAGE_KEY, LEGACY_STORAGE_KEY);
    import("@/lib/store")
      .then(({ migrateFromLocalStorage }) =>
        migrateFromLocalStorage<ScanPreferences>(
          STORAGE_KEY,
          STORE_KEY,
          DEFAULTS,
          parseScanPreferences,
        ),
      )
      .then((stored) => {
        setPrefsState(parseScanPreferences(stored) ?? DEFAULTS);
      })
      .catch(() => {});
  }, []);

  const enabledCategories = useMemo(
    () =>
      Object.entries(prefs.categories)
        .filter(([_, enabled]) => enabled)
        .map(([cat]) => cat),
    [prefs.categories],
  );

  const value = useMemo(
    () => ({ prefs, setPrefs, enabledCategories }),
    [prefs, setPrefs, enabledCategories],
  );

  return <ScanPrefsContext.Provider value={value}>{children}</ScanPrefsContext.Provider>;
}
