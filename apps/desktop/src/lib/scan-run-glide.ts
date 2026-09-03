import type { ScanRunStepKind } from "@/lib/scan-progress-model";
import { readScanRunProgress } from "@/lib/scan-progress-store";

const TICK_MS = 50;
/** Share of the remaining gap closed per tick: about a 200 ms time constant. */
const CATCH_UP_SHARE = 0.22;
/** Hard cap of 80 points per second, so a milestone lands as a quick glide, not a cut. */
const MAX_STEP_PER_TICK = 4;
/** Floor on each step, so the tail of a glide finishes instead of crawling forever. */
const MIN_STEP_PER_TICK = 0.15;
const SNAP_WITHIN = 0.2;

/** One tick of the displayed percent toward the model's estimate. */
export function stepToward(current: number, target: number): number {
  const delta = target - current;
  if (Math.abs(delta) <= SNAP_WITHIN) return target;
  const eased = delta * CATCH_UP_SHARE;
  const bounded = Math.sign(delta) * Math.min(Math.abs(eased), MAX_STEP_PER_TICK);
  const step =
    Math.abs(bounded) < MIN_STEP_PER_TICK
      ? Math.sign(delta) * Math.min(Math.abs(delta), MIN_STEP_PER_TICK)
      : bounded;
  return current + step;
}

interface DisplayedProgress {
  step: ScanRunStepKind;
  percent: number;
}

// One glide clock for every consumer, held outside React: the ring, the bar,
// and the footer read the same number from the same 50 ms tick, the clock
// runs only while something is painting the percent, and a tick that moves
// nothing wakes nobody. The hooks in components/scan select from it.
let displayed: DisplayedProgress = { step: "web", percent: 0 };
/** True while `displayed` is frozen at the live value because no clock is running. */
let parked = false;
let timerId: number | null = null;
const listeners = new Set<() => void>();

function notify() {
  for (const listener of listeners) listener();
}

function snapToLive() {
  const live = readScanRunProgress();
  if (live.step === displayed.step && live.percent === displayed.percent) return;
  displayed = { step: live.step, percent: live.percent };
  notify();
}

function tick() {
  const live = readScanRunProgress();
  // A step change (Web Scan to Code Scan) is a new ring, so the display snaps
  // to it rather than gliding down from 100.
  const percent =
    live.step === displayed.step ? stepToward(displayed.percent, live.percent) : live.percent;
  if (live.step === displayed.step && percent === displayed.percent) return;
  displayed = { step: live.step, percent };
  notify();
}

function startClock() {
  if (timerId != null) return;
  parked = false;
  snapToLive();
  timerId = window.setInterval(tick, TICK_MS);
}

function stopClock() {
  if (timerId == null) return;
  window.clearInterval(timerId);
  timerId = null;
  parked = false;
}

function onVisibilityChange() {
  // Nothing is painted while the window is hidden, so the clock stops. On
  // return the display snaps to wherever the run model got to.
  if (document.visibilityState === "hidden") stopClock();
  else startClock();
}

/** Subscribe a consumer to the glide clock; the clock runs only while someone is subscribed. */
export function subscribeScanRunGlide(listener: () => void): () => void {
  listeners.add(listener);
  if (listeners.size === 1) {
    document.addEventListener("visibilitychange", onVisibilityChange);
    if (document.visibilityState !== "hidden") startClock();
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      document.removeEventListener("visibilitychange", onVisibilityChange);
      stopClock();
    }
  };
}

function current(): DisplayedProgress {
  // With no clock running, the first read freezes the live value, so a fresh
  // mount (or one restored from the background) starts at the model's number
  // and every read within one render agrees.
  if (timerId == null && !parked) {
    const live = readScanRunProgress();
    displayed = { step: live.step, percent: live.percent };
    parked = true;
  }
  return displayed;
}

/** The displayed percent as a smooth, fractional value that changes on every tick. */
export function getScanRunGlidePercent(): number {
  return current().percent;
}

/** The displayed percent rounded to a whole number. */
export function getScanRunGlideWholePercent(): number {
  return Math.round(current().percent);
}
