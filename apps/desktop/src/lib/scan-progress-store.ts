import { useSyncExternalStore } from "react";
import type { MultiScanProgressEvent, ScanProgressEvent } from "@/hooks/useScan";

export interface ScanProgressSnapshot {
  progress: ScanProgressEvent | null;
  multiProgress: MultiScanProgressEvent | null;
}

let progress: ScanProgressEvent | null = null;
let multiProgress: MultiScanProgressEvent | null = null;
let snapshot: ScanProgressSnapshot = { progress, multiProgress };
const listeners = new Set<() => void>();

function publish() {
  snapshot = { progress, multiProgress };
  for (const fn of listeners) fn();
}

export function publishScanProgress(next: ScanProgressEvent | null) {
  progress = next;
  publish();
}

export function publishMultiScanProgress(next: MultiScanProgressEvent | null) {
  multiProgress = next;
  publish();
}

/** Clear both channels at scan start/reset. No-op (no publish) when already
 * clear, so an idle reset does not wake subscribers. */
export function resetScanProgress() {
  if (progress === null && multiProgress === null) return;
  progress = null;
  multiProgress = null;
  publish();
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
