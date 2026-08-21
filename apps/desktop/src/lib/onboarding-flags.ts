// One-shot onboarding flags persist across reloads.

import { useSyncExternalStore } from "react";

const FIRST_SCAN_COMPLETED_KEY = "sitecmd:onboarding:first-scan-completed";
const STORAGE_EVENT = "sitecmd:onboarding-flags-changed";

function readBooleanFlag(key: string): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}

function writeBooleanFlag(key: string, value: boolean) {
  if (typeof window === "undefined") return;
  try {
    if (value) window.localStorage.setItem(key, "1");
    else window.localStorage.removeItem(key);
    window.dispatchEvent(new Event(STORAGE_EVENT));
  } catch {
    // best effort
  }
}

export function readHasCompletedFirstScan(): boolean {
  return readBooleanFlag(FIRST_SCAN_COMPLETED_KEY);
}

export function markFirstScanCompleted() {
  if (readBooleanFlag(FIRST_SCAN_COMPLETED_KEY)) return;
  writeBooleanFlag(FIRST_SCAN_COMPLETED_KEY, true);
}

export function clearFirstScanCompletedForTests() {
  writeBooleanFlag(FIRST_SCAN_COMPLETED_KEY, false);
}

function subscribeToFlagChanges(callback: () => void): () => void {
  if (typeof window === "undefined") return () => {};
  window.addEventListener(STORAGE_EVENT, callback);
  window.addEventListener("storage", callback);
  return () => {
    window.removeEventListener(STORAGE_EVENT, callback);
    window.removeEventListener("storage", callback);
  };
}

export function useHasCompletedFirstScan(): boolean {
  return useSyncExternalStore(subscribeToFlagChanges, readHasCompletedFirstScan, () => false);
}
