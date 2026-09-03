import { useSyncExternalStore } from "react";
import type { MultiScanProgressEvent, ScanProgressEvent } from "@/hooks/useScan";
import {
  advanceScanRunPages,
  advanceScanRunTracks,
  createScanRunTracks,
  estimateScanRunProgress,
  type ScanRunPlan,
  type ScanRunProgressEstimate,
  type ScanRunStepKind,
  type ScanRunTracks,
} from "@/lib/scan-progress-model";

export interface ScanProgressSnapshot {
  progress: ScanProgressEvent | null;
  multiProgress: MultiScanProgressEvent | null;
}

let progress: ScanProgressEvent | null = null;
let multiProgress: MultiScanProgressEvent | null = null;
let snapshot: ScanProgressSnapshot = { progress, multiProgress };
const listeners = new Set<() => void>();

// The run's percent lives here rather than in any one consumer so the overlay
// ring, the jobs tray, and the system tray all show the same number, and so
// backgrounding the overlay does not restart it.
let tracks: ScanRunTracks | null = null;
let highWater = 0;
let highWaterStep: ScanRunStepKind | null = null;

const IDLE_PROGRESS: ScanRunProgressEstimate = { step: "web", percent: 0 };

function publish() {
  snapshot = { progress, multiProgress };
  for (const fn of listeners) fn();
}

/** Start a run's progress model. Call before the first progress event of a scan. */
export function beginScanRun(plan: ScanRunPlan) {
  tracks = createScanRunTracks(plan, Date.now());
  highWater = 0;
  highWaterStep = null;
  clearScanProgress();
}

/** A run that was never announced still gets a model shaped by its first event. */
function ensureTracks(plan: ScanRunPlan): ScanRunTracks {
  if (!tracks) {
    tracks = createScanRunTracks(plan, Date.now());
    highWater = 0;
    highWaterStep = null;
  }
  return tracks;
}

export function publishScanProgress(next: ScanProgressEvent | null) {
  if (next) {
    const fallback: ScanRunPlan = next.check_id.startsWith("code-scan.")
      ? { web: null, code: true }
      : { web: "health", code: false };
    tracks = advanceScanRunTracks(ensureTracks(fallback), next, Date.now());
  }
  progress = next;
  publish();
}

export function publishMultiScanProgress(next: MultiScanProgressEvent | null) {
  if (next) {
    const fallback: ScanRunPlan = { web: "health", code: false, pageCount: next.page_count };
    tracks = advanceScanRunPages(ensureTracks(fallback), next, Date.now());
  }
  multiProgress = next;
  publish();
}

/**
 * The active step and its percent right now. Monotonic for the life of a
 * step: a plan change or a late event can lower the model's estimate, and the
 * number a person is watching must never tick backward. A new step (Web Scan
 * to Code Scan) starts its own ring from the model's value.
 */
export function readScanRunProgress(nowMs: number = Date.now()): ScanRunProgressEstimate {
  if (!tracks) return IDLE_PROGRESS;
  const estimate = estimateScanRunProgress(tracks, nowMs);
  if (estimate.step !== highWaterStep) {
    highWaterStep = estimate.step;
    highWater = 0;
  }
  if (estimate.percent > highWater) highWater = estimate.percent;
  return { step: estimate.step, percent: highWater };
}

export function readScanRunPercent(nowMs: number = Date.now()): number {
  return readScanRunProgress(nowMs).percent;
}

function clearScanProgress() {
  if (progress === null && multiProgress === null) return;
  progress = null;
  multiProgress = null;
  publish();
}

/** Clear both channels and the run model at scan start/reset. No-op publish
 * when already clear, so an idle reset does not wake subscribers. */
export function resetScanProgress() {
  tracks = null;
  highWater = 0;
  highWaterStep = null;
  clearScanProgress();
}

export function getScanProgressSnapshot(): ScanProgressSnapshot {
  return snapshot;
}

function subscribe(callback: () => void) {
  listeners.add(callback);
  return () => {
    listeners.delete(callback);
  };
}

/** Imperative subscription for non-render consumers (returns an unsubscribe). */
export const subscribeScanProgress = subscribe;

/** React hook for render consumers (the scan overlay) that must repaint on
 * every tick. */
export function useScanProgress(): ScanProgressSnapshot {
  return useSyncExternalStore(subscribe, getScanProgressSnapshot);
}
