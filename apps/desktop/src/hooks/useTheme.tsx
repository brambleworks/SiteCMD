import {
  useState,
  useEffect,
  useCallback,
  useMemo,
  useRef,
  createContext,
  useContext,
} from "react";
import {
  migrateLegacyValue,
  readCurrentOrLegacyValue,
  writeCurrentValue,
} from "@/lib/local-storage-migration";

type Theme = "light" | "dark" | "system";

// User-selectable themes stay disabled until light mode ships.
const LOCKED_THEME: "dark" | null = "dark";

interface ThemeContextValue {
  theme: Theme;
  setTheme: (t: Theme) => void;
  resolved: "light" | "dark";
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: "system",
  setTheme: () => {},
  resolved: "light",
});

export function useTheme() {
  return useContext(ThemeContext);
}

function getSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function resolve(theme: Theme): "light" | "dark" {
  return theme === "system" ? getSystemTheme() : theme;
}

function apply(resolved: "light" | "dark") {
  document.documentElement.classList.toggle("dark", resolved === "dark");
}

const STORAGE_KEY = "sitecmd-theme";
const LEGACY_STORAGE_KEY = "sitehealthkit-theme";
const STORE_KEY = "theme";

function parseTheme(value: unknown): Theme | null {
  return value === "light" || value === "dark" || value === "system" ? value : null;
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(() => {
    if (LOCKED_THEME) return LOCKED_THEME;
    const stored = readCurrentOrLegacyValue(localStorage, STORAGE_KEY, LEGACY_STORAGE_KEY);
    return stored === "light" || stored === "dark" || stored === "system" ? stored : "system";
  });
  const [resolved, setResolved] = useState<"light" | "dark">(() => LOCKED_THEME ?? resolve(theme));

  const setTheme = useCallback((t: Theme) => {
    if (LOCKED_THEME) return;
    // Dual-write: localStorage for flash-prevention script, Tauri store for durability
    writeCurrentValue(localStorage, STORAGE_KEY, LEGACY_STORAGE_KEY, t);
    import("@/lib/store").then(({ storeSet }) => storeSet(STORE_KEY, t)).catch(() => {});
    setThemeState(t);
    const r = resolve(t);
    setResolved(r);
    apply(r);
  }, []);

  // Hydrate once from the Tauri store, which outranks localStorage.
  const hasBootstrappedRef = useRef(false);
  useEffect(() => {
    if (hasBootstrappedRef.current) return;
    hasBootstrappedRef.current = true;
    if (LOCKED_THEME) {
      apply(LOCKED_THEME);
      return;
    }
    apply(resolve(theme));
    migrateLegacyValue(localStorage, STORAGE_KEY, LEGACY_STORAGE_KEY);
    import("@/lib/store")
      .then(({ migrateFromLocalStorage }) =>
        migrateFromLocalStorage<Theme>(STORAGE_KEY, STORE_KEY, "system", parseTheme),
      )
      .then((stored) => {
        if (stored !== theme) {
          setThemeState(stored);
          const r = resolve(stored);
          setResolved(r);
          apply(r);
          writeCurrentValue(localStorage, STORAGE_KEY, LEGACY_STORAGE_KEY, stored);
        }
      })
      .catch(() => {});
  }, [theme]);

  // Listen for system theme changes when in "system" mode
  useEffect(() => {
    if (LOCKED_THEME) return;
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      const r = getSystemTheme();
      setResolved(r);
      apply(r);
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  const value = useMemo(() => ({ theme, setTheme, resolved }), [theme, setTheme, resolved]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
