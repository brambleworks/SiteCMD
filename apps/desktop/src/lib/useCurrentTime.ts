import { useSyncExternalStore } from "react";
import { MS_PER_MINUTE } from "./format";

let currentTimeMs = Date.now();
let intervalId: number | null = null;
const listeners = new Set<() => void>();

function updateCurrentTime() {
  currentTimeMs = Date.now();
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
  if (listeners.size === 0) {
    currentTimeMs = Date.now();
    intervalId = window.setInterval(updateCurrentTime, MS_PER_MINUTE);
  }
  listeners.add(listener);

  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && intervalId !== null) {
      window.clearInterval(intervalId);
      intervalId = null;
    }
  };
}

function getSnapshot() {
  return currentTimeMs;
}

export function useCurrentTime(): number {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
