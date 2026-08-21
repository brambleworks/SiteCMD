import { useSyncExternalStore } from "react";
import type { AppTarget } from "@/lib/app-targets";

type RestoreScanJobTarget = {
  restoreScan: true;
  projectId?: number | null;
  url?: string | null;
};

export type BackgroundJobTarget = AppTarget | RestoreScanJobTarget;

export interface BackgroundJob {
  id: string;
  type: "scan" | "probes" | "sync";
  label: string;
  /** Site or environment this job belongs to */
  scopeLabel?: string;
  /** 0–100 percentage, or undefined for indeterminate */
  progress?: number;
  /** Extra detail text (e.g., "23 of 45 checks") */
  detail?: string;
  /** Where this job should take the user when opened */
  target?: BackgroundJobTarget | null;
  startedAt: number;
  endedAt?: number;
  status: "running" | "success" | "error";
}

const MAX_RECENT_JOBS = 6;
const RECENT_TTL_MS = 15 * 60 * 1000;

let runningJobs: BackgroundJob[] = [];
let recentJobs: BackgroundJob[] = [];
let snapshot = { running: runningJobs, recent: recentJobs };
const listeners = new Set<() => void>();

function publish() {
  snapshot = { running: runningJobs, recent: recentJobs };
  for (const fn of listeners) fn();
}

function pruneRecent() {
  const now = Date.now();
  recentJobs = recentJobs
    .filter((job) => (job.endedAt ?? now) > now - RECENT_TTL_MS)
    .slice(0, MAX_RECENT_JOBS);
}

/** Shallow value-equality for a job target (plain serializable data, one level
 * deep). Lets addJob skip a redundant publish when nothing visible changed. */
function jobTargetsEqual(
  a: BackgroundJobTarget | null | undefined,
  b: BackgroundJobTarget | null | undefined,
) {
  if (a == null || b == null) return a == b;
  const aKeys = Object.keys(a) as (keyof typeof a)[];
  const bKeys = Object.keys(b);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((key) => a[key] === (b as Record<string, unknown>)[key]);
}

function sameRunningJob(
  existing: BackgroundJob | undefined,
  next: Omit<BackgroundJob, "startedAt" | "status" | "endedAt">,
): boolean {
  return (
    existing != null &&
    existing.status === "running" &&
    existing.type === next.type &&
    existing.label === next.label &&
    existing.scopeLabel === next.scopeLabel &&
    existing.progress === next.progress &&
    existing.detail === next.detail &&
    jobTargetsEqual(existing.target, next.target)
  );
}

export function addJob(job: Omit<BackgroundJob, "startedAt" | "status" | "endedAt">) {
  const existing = runningJobs.find((entry) => entry.id === job.id);
  // Ignore unchanged progress ticks to avoid store-wide re-renders.
  if (sameRunningJob(existing, job)) return;
  runningJobs = runningJobs.filter((entry) => entry.id !== job.id);
  recentJobs = recentJobs.filter((entry) => entry.id !== job.id);
  runningJobs = [
    ...runningJobs,
    {
      ...job,
      startedAt: existing?.startedAt ?? Date.now(),
      status: "running",
    },
  ];
  pruneRecent();
  publish();
}

export function updateJob(
  id: string,
  updates: Partial<Pick<BackgroundJob, "label" | "scopeLabel" | "progress" | "detail" | "target">>,
) {
  runningJobs = runningJobs.map((job) => (job.id === id ? { ...job, ...updates } : job));
  publish();
}

export function removeJob(id: string) {
  runningJobs = runningJobs.filter((job) => job.id !== id);
  recentJobs = recentJobs.filter((job) => job.id !== id);
  pruneRecent();
  publish();
}

export function removeRunningJob(id: string) {
  runningJobs = runningJobs.filter((job) => job.id !== id);
  publish();
}

export function clearJobsByType(type: BackgroundJob["type"]) {
  runningJobs = runningJobs.filter((job) => job.type !== type);
  recentJobs = recentJobs.filter((job) => job.type !== type);
  pruneRecent();
  publish();
}

export function completeJob(
  id: string,
  updates?: Partial<Omit<BackgroundJob, "id" | "startedAt" | "status">>,
) {
  const existing = runningJobs.find((job) => job.id === id);
  if (!existing) return;
  runningJobs = runningJobs.filter((job) => job.id !== id);
  recentJobs = [
    {
      ...existing,
      ...updates,
      endedAt: Date.now(),
      progress: updates?.progress ?? 100,
      status: "success",
    },
    ...recentJobs.filter((job) => job.id !== id),
  ];
  pruneRecent();
  publish();
}

export function recordCompletedJob(
  job: Omit<BackgroundJob, "startedAt" | "endedAt" | "status"> & {
    startedAt?: number;
    endedAt?: number;
  },
) {
  runningJobs = runningJobs.filter((entry) => entry.id !== job.id);
  recentJobs = [
    {
      ...job,
      startedAt: job.startedAt ?? Date.now(),
      endedAt: job.endedAt ?? Date.now(),
      progress: job.progress ?? 100,
      status: "success",
    },
    ...recentJobs.filter((entry) => entry.id !== job.id),
  ];
  pruneRecent();
  publish();
}

export function failJob(
  id: string,
  updates?: Partial<Omit<BackgroundJob, "id" | "startedAt" | "status">>,
) {
  const existing = runningJobs.find((job) => job.id === id);
  if (!existing) return;
  runningJobs = runningJobs.filter((job) => job.id !== id);
  recentJobs = [
    {
      ...existing,
      ...updates,
      endedAt: Date.now(),
      status: "error",
    },
    ...recentJobs.filter((job) => job.id !== id),
  ];
  pruneRecent();
  publish();
}

function subscribe(callback: () => void) {
  listeners.add(callback);
  return () => {
    listeners.delete(callback);
  };
}

function getSnapshot() {
  return snapshot;
}

export function useJobs(): BackgroundJob[] {
  return useSyncExternalStore(subscribe, () => getSnapshot().running);
}

export function useJobsCenter(): { running: BackgroundJob[]; recent: BackgroundJob[] } {
  return useSyncExternalStore(subscribe, getSnapshot);
}

function getRunningCount() {
  return snapshot.running.length;
}

/** Subscribe to job counts without high-frequency progress updates. */
export function useRunningJobsCount(): number {
  return useSyncExternalStore(subscribe, getRunningCount);
}
