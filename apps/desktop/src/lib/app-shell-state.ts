import { parseJsonRecord } from "./json-record";

const APP_SHELL_STATE_KEY = "sitecmd_shell_state_v1";

const KNOWN_APP_PAGES = new Set([
  "dashboard",
  "security",
  "updates",
  "search-console",
  "analytics",
  "integrations",
  "events",
  "deploys",
  "issues",
  "alerts",
  "sites",
  "settings",
  "reports",
]);

const STARTUP_PAGE_ALIASES: Partial<Record<string, string>> = {
  today: "dashboard",
  sites: "dashboard",
  scans: "issues",
};

export function readPersistedShellPage(): string | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(APP_SHELL_STATE_KEY);
    if (!raw) return null;
    const parsed = parseJsonRecord(raw);
    const storedPage = typeof parsed?.page === "string" ? parsed.page : null;
    const page = storedPage ? (STARTUP_PAGE_ALIASES[storedPage] ?? storedPage) : null;
    return page && KNOWN_APP_PAGES.has(page) ? page : null;
  } catch {
    return null;
  }
}

export function writePersistedShellPage(page: string) {
  if (typeof window === "undefined" || !KNOWN_APP_PAGES.has(page)) return;
  try {
    const storedPage = STARTUP_PAGE_ALIASES[page] ?? page;
    window.localStorage.setItem(APP_SHELL_STATE_KEY, JSON.stringify({ page: storedPage }));
  } catch {
    // best effort
  }
}

export function clearPersistedShellPage() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(APP_SHELL_STATE_KEY);
  } catch {
    // best effort
  }
}
