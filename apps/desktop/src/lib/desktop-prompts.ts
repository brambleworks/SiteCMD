import { useSyncExternalStore } from "react";
import { storeSet, migrateFromLocalStorage } from "@/lib/store";
import {
  normalizeAppUrlForKey,
  normalizeHttpTargetUrl,
  type AppTarget,
  type AppTargetPage,
} from "@/lib/app-targets";
import { isJsonRecord } from "@/lib/json-record";
import {
  getDesktopWatchImpactSentenceForReason,
  normalizeDesktopWatchReason,
  type DesktopWatchPromptPage,
} from "@/lib/desktop-watch-reasons";

type DesktopPromptPage = Extract<AppTargetPage, DesktopWatchPromptPage>;

const DESKTOP_PROMPT_PAGES = new Set<DesktopPromptPage>(["search-console", "updates", "issues"]);

export interface DesktopPromptEntry {
  id: string;
  projectId: number;
  url: string;
  page: DesktopPromptPage;
  focus?: string | null;
  title: string;
  detail: string;
  relativePath: string;
  absolutePath?: string | null;
  kind: string;
  createdAt: number;
  updatedAt: number;
}

interface DesktopWatchPromptMemoryCue {
  label: string;
  tone: "regressed" | "verified" | "new";
  domainLabel?: string | null;
}

interface DesktopWatchPromptCopy {
  title: string;
  detail: string;
}

const STORAGE_KEY = "sitecmd_desktop_prompts_v1";
const STORE_KEY = "desktop-prompts";
const MAX_ENTRIES = 40;

let entries: DesktopPromptEntry[] = [];
let loaded = false;
let snapshot = entries;
const listeners = new Set<() => void>();

migrateFromLocalStorage<DesktopPromptEntry[]>(STORAGE_KEY, STORE_KEY, [], parseDesktopPromptEntries)
  .then((stored) => {
    if (Array.isArray(stored) && stored.length > 0 && !loaded) {
      entries = stored.slice(0, MAX_ENTRIES);
      snapshot = entries;
      loaded = true;
      for (const listener of listeners) listener();
    }
  })
  .catch(() => {});

function normalizeUrl(url: string): string {
  return normalizeAppUrlForKey(url);
}

export function normalizeDesktopPromptReason(kind: string, page: DesktopPromptPage): string {
  return normalizeDesktopWatchReason(kind, page);
}

function withPeriod(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  return /[.!?]$/.test(trimmed) ? trimmed : `${trimmed}.`;
}

function getDesktopWatchImpactSentence(
  reason: string,
  page: DesktopPromptPage,
  focus?: string | null,
): string | null {
  return getDesktopWatchImpactSentenceForReason({ reason, page, focus });
}

function getDesktopWatchHistorySentence(
  memoryCue?: DesktopWatchPromptMemoryCue | null,
): string | null {
  if (!memoryCue?.label) return null;
  const domainSuffix = memoryCue.domainLabel ? ` in ${memoryCue.domainLabel}` : "";
  return `History: ${memoryCue.label}${domainSuffix}.`;
}

export function buildDesktopWatchPromptCopy(options: {
  title: string;
  detail: string;
  page: DesktopPromptPage;
  reason: string;
  focus?: string | null;
  relativePath: string;
  nextActionLabel?: string | null;
  memoryCue?: DesktopWatchPromptMemoryCue | null;
}): DesktopWatchPromptCopy {
  const regressedDomain =
    options.memoryCue?.tone === "regressed" ? (options.memoryCue.domainLabel ?? "Code") : null;

  const title = regressedDomain ? `${options.title} - ${regressedDomain} regressed` : options.title;

  const detail = [
    withPeriod(options.detail),
    `Changed file: ${options.relativePath}.`,
    getDesktopWatchImpactSentence(options.reason, options.page, options.focus),
    getDesktopWatchHistorySentence(options.memoryCue),
    options.nextActionLabel ? `Recommended next step: ${options.nextActionLabel}.` : null,
  ]
    .filter((part): part is string => Boolean(part))
    .join(" ");

  return { title, detail };
}

function publish() {
  snapshot = entries;
  for (const listener of listeners) listener();
}

function persist() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } catch {
    // best effort
  }
  storeSet(STORE_KEY, entries).catch(() => {});
}

function ensureLoaded() {
  if (loaded || typeof window === "undefined") return;
  loaded = true;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return;
    const parsed = parseDesktopPromptEntries(JSON.parse(raw) as unknown);
    if (!parsed) return;
    entries = parsed
      .map((entry) => ({ ...entry, url: normalizeUrl(entry.url) }))
      .sort((a, b) => b.updatedAt - a.updatedAt)
      .slice(0, MAX_ENTRIES);
    snapshot = entries;
  } catch {
    entries = [];
    snapshot = entries;
  }
}

function parseDesktopPromptEntry(value: unknown): DesktopPromptEntry | null {
  if (!isJsonRecord(value)) return null;
  const projectId = parsePositiveInteger(value.projectId);
  const url = normalizeHttpTargetUrl(typeof value.url === "string" ? value.url : null);
  const page = parseDesktopPromptPage(value.page);
  const createdAt = parseTimestamp(value.createdAt);
  const updatedAt = parseTimestamp(value.updatedAt);
  if (
    typeof value.id !== "string" ||
    projectId == null ||
    !url ||
    !page ||
    typeof value.title !== "string" ||
    typeof value.detail !== "string" ||
    typeof value.relativePath !== "string" ||
    typeof value.kind !== "string" ||
    createdAt == null ||
    updatedAt == null
  ) {
    return null;
  }
  return {
    id: value.id,
    projectId,
    url,
    page,
    focus: typeof value.focus === "string" ? value.focus : null,
    title: value.title,
    detail: value.detail,
    relativePath: value.relativePath,
    absolutePath: typeof value.absolutePath === "string" ? value.absolutePath : null,
    kind: value.kind,
    createdAt,
    updatedAt,
  };
}

function parseDesktopPromptEntries(value: unknown): DesktopPromptEntry[] | null {
  if (!Array.isArray(value)) return null;
  return value.flatMap((entry) => {
    const parsed = parseDesktopPromptEntry(entry);
    return parsed ? [parsed] : [];
  });
}

function parseDesktopPromptPage(value: unknown): DesktopPromptPage | null {
  return typeof value === "string" && DESKTOP_PROMPT_PAGES.has(value as DesktopPromptPage)
    ? (value as DesktopPromptPage)
    : null;
}

function parsePositiveInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 ? value : null;
}

function parseTimestamp(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

export function buildDesktopPromptId(
  projectId: number,
  url: string,
  kind: string,
  relativePath: string,
): string {
  return `${projectId}:${normalizeUrl(url)}:${kind}:${relativePath}`;
}

export function queueDesktopPrompt(
  entry: Omit<DesktopPromptEntry, "id" | "createdAt" | "updatedAt">,
) {
  ensureLoaded();
  const now = Date.now();
  const id = buildDesktopPromptId(entry.projectId, entry.url, entry.kind, entry.relativePath);
  const existing = entries.find((candidate) => candidate.id === id);
  entries = [
    {
      ...existing,
      ...entry,
      id,
      url: normalizeUrl(entry.url),
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
    },
    ...entries.filter((candidate) => candidate.id !== id),
  ].slice(0, MAX_ENTRIES);
  persist();
  publish();
}

export function resolveDesktopPrompt(id: string) {
  ensureLoaded();
  entries = entries.filter((entry) => entry.id !== id);
  persist();
  publish();
}

export function clearDesktopPrompts(filter?: { projectId?: number; url?: string }) {
  ensureLoaded();
  if (!filter) {
    entries = [];
    persist();
    publish();
    return;
  }
  const normalizedUrl = filter.url ? normalizeUrl(filter.url) : null;
  entries = entries.filter((entry) => {
    if (filter.projectId != null && entry.projectId !== filter.projectId) return true;
    if (normalizedUrl && entry.url !== normalizedUrl) return true;
    return false;
  });
  persist();
  publish();
}

function subscribe(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

function getSnapshot() {
  ensureLoaded();
  return snapshot;
}

export function useDesktopPromptCenter(): DesktopPromptEntry[] {
  return useSyncExternalStore(subscribe, getSnapshot);
}

export function buildDesktopPromptTarget(entry: DesktopPromptEntry): AppTarget {
  return {
    page: entry.page,
    projectId: entry.projectId,
    url: normalizeUrl(entry.url),
    focus: entry.focus ?? null,
    promptId: entry.id,
    reason: normalizeDesktopPromptReason(entry.kind, entry.page),
    filePath: entry.absolutePath ?? null,
  };
}

export function getDesktopPromptById(id: string): DesktopPromptEntry | null {
  ensureLoaded();
  return entries.find((entry) => entry.id === id) ?? null;
}

export function getLatestDesktopPrompt(
  promptEntries: DesktopPromptEntry[],
  filter: {
    projectId: number;
    url?: string | null;
    page?: DesktopPromptPage;
    focus?: string | null;
  },
): DesktopPromptEntry | null {
  const normalizedUrl = filter.url ? normalizeUrl(filter.url) : null;
  return (
    promptEntries.find((entry) => {
      if (entry.projectId !== filter.projectId) return false;
      if (normalizedUrl && entry.url !== normalizedUrl) return false;
      if (filter.page && entry.page !== filter.page) return false;
      if (filter.focus && entry.focus !== filter.focus) return false;
      return true;
    }) ?? null
  );
}
