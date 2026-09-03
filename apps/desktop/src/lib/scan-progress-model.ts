import type { MultiScanProgressEvent, ScanProgressEvent } from "@/hooks/useScan";

// The scan pipeline reports milestones, not time. A Web Scan fires ~90
// in-memory check events inside half a second, then goes quiet for one to
// thirteen seconds while the origin checks wait on the network, then fires
// the rest in one burst. Code Scan reports a hand-placed 0-100 ladder per
// stage. Mapping those events straight onto a percentage produces a number
// that sprints, freezes, and leaps. This model instead keeps one monotonic
// estimate per step: every event sets a confirmed floor, and between events
// the estimate drifts toward the end of the current phase on a hyperbolic
// curve, so the number always moves and never claims a phase finished before
// its event arrives. Each step of a full scan (Web Scan, then Code Scan)
// fills its own 0 to 100 ring, because the overlay presents them as separate
// steps with their own headings and stage grids.

export type WebScanFocus = "health" | "security" | "accessibility" | "polish";

const WEB_SCAN_FOCUSES: readonly string[] = ["health", "security", "accessibility", "polish"];

/** The web focus a scan-type string names, defaulting to a health scan. */
export function asWebScanFocus(value: string | null | undefined): WebScanFocus {
  return value && WEB_SCAN_FOCUSES.includes(value) ? (value as WebScanFocus) : "health";
}

export interface ScanRunPlan {
  /** Focus of the web collector, or null when the run has no web step. */
  web: WebScanFocus | null;
  /** Whether a Code Scan step is part of the run. */
  code: boolean;
  /** Pages the web collector will visit; defaults to one. */
  pageCount?: number;
}

type WebPhase = "fetch" | "checks" | "polish" | "browser";

const WEB_PHASE_ORDER: readonly WebPhase[] = ["fetch", "checks", "polish", "browser"];

/**
 * Where each phase ends, as a fraction of one page's web collector. The
 * weights follow measured wall time rather than event counts: the checks
 * phase is mostly a half-second sprint plus a network wait, and the browser
 * pass is the longest single step.
 */
const WEB_PHASE_ENDS: Record<WebScanFocus, Partial<Record<WebPhase, number>>> = {
  health: { fetch: 0.1, checks: 0.6, polish: 0.68, browser: 1 },
  accessibility: { fetch: 0.12, checks: 0.62, browser: 1 },
  security: { fetch: 0.2, checks: 1 },
  polish: { fetch: 0.25, polish: 0.55, browser: 1 },
};

/** Milliseconds for the drift to cover half of what remains of a phase. */
const WEB_PHASE_HALF_LIFE_MS: Record<WebPhase, number> = {
  fetch: 1500,
  checks: 1500,
  polish: 1000,
  browser: 1000,
};

/** Drift budget while one collector hands off to the next. */
const HANDOFF_HALF_LIFE_MS = 1500;
const HANDOFF_CEILING = 0.05;

const CODE_SCAN_PREFIX = "code-scan.";

/** Where each Code Scan stage ends on the backend's 0-100 ladder. */
const CODE_STAGE_ENDS: Record<string, number> = {
  "collect-files": 0.15,
  "analyze-source": 0.55,
  "supply-chain": 0.68,
  operations: 0.8,
  "ai-scaffolding": 0.82,
  finalize: 0.86,
  save: 0.9,
  "work-items": 0.94,
  summary: 1,
  complete: 1,
};

const CODE_STAGE_HALF_LIFE_MS: Record<string, number> = {
  "collect-files": 1500,
  "analyze-source": 4000,
  "supply-chain": 3000,
  operations: 2000,
};
const CODE_STAGE_DEFAULT_HALF_LIFE_MS = 1200;
const CODE_UNKNOWN_STAGE_HEADROOM = 0.05;

/** One collector's confirmed progress plus the drift that fills the wait for its next event. */
interface Drift {
  /** Confirmed fraction of the collector. Never decreases. */
  floor: number;
  /** Fraction the estimate approaches while no event arrives; never below `floor`. */
  ceiling: number;
  /** Milliseconds for the drift to cover half of the remaining gap. */
  halfLife: number;
  /** When `floor` was last set. */
  since: number;
}

export type ScanRunStepKind = "web" | "code";

export interface ScanRunProgressEstimate {
  /** Which step's ring the percent belongs to. */
  step: ScanRunStepKind;
  percent: number;
}

export interface ScanRunTracks {
  plan: ScanRunPlan;
  /** The page currently being scanned. */
  web: Drift;
  webDone: boolean;
  pageIndex: number;
  pageCount: number;
  /** The current page has a terminal status, so it counts as whole. */
  pageSettled: boolean;
  code: Drift;
  /** A code event has arrived, so the Code Scan step owns the ring. */
  codeSeen: boolean;
  codeDone: boolean;
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
}

function driftValue(drift: Drift, nowMs: number): number {
  if (drift.ceiling <= drift.floor) return drift.floor;
  const elapsed = Math.max(0, nowMs - drift.since);
  // Hyperbolic approach: half the gap at one half-life, 90% at nine. It keeps
  // moving through a long stall instead of flat-lining the way an exponential
  // curve would, and it never reaches the ceiling on its own.
  const approach = elapsed / (elapsed + drift.halfLife);
  return drift.floor + (drift.ceiling - drift.floor) * approach;
}

/** Bank whatever the drift already showed, then raise the floor to the new milestone. */
function settle(
  drift: Drift,
  floor: number,
  ceiling: number,
  halfLife: number,
  nowMs: number,
): Drift {
  const banked = Math.max(driftValue(drift, nowMs), clamp01(floor));
  return { floor: banked, ceiling: Math.max(banked, clamp01(ceiling)), halfLife, since: nowMs };
}

function firstWebPhase(focus: WebScanFocus): { end: number; halfLife: number } {
  for (const phase of WEB_PHASE_ORDER) {
    const end = WEB_PHASE_ENDS[focus][phase];
    if (end !== undefined) return { end, halfLife: WEB_PHASE_HALF_LIFE_MS[phase] };
  }
  return { end: 1, halfLife: HANDOFF_HALF_LIFE_MS };
}

function freshWebDrift(focus: WebScanFocus | null, nowMs: number): Drift {
  const first = focus ? firstWebPhase(focus) : { end: 0, halfLife: HANDOFF_HALF_LIFE_MS };
  return { floor: 0, ceiling: first.end, halfLife: first.halfLife, since: nowMs };
}

function freshCodeDrift(nowMs: number): Drift {
  return { floor: 0, ceiling: HANDOFF_CEILING, halfLife: HANDOFF_HALF_LIFE_MS, since: nowMs };
}

export function createScanRunTracks(plan: ScanRunPlan, nowMs: number): ScanRunTracks {
  return {
    plan,
    web: freshWebDrift(plan.web, nowMs),
    webDone: plan.web === null,
    pageIndex: 0,
    pageCount: Math.max(1, plan.pageCount ?? 1),
    pageSettled: false,
    code: freshCodeDrift(nowMs),
    codeSeen: false,
    codeDone: false,
  };
}

interface WebPhaseBounds {
  start: number;
  end: number;
  /** End of the next planned phase, so a finished phase can drift into it. */
  nextEnd: number;
  nextHalfLife: number;
}

function webPhaseBounds(focus: WebScanFocus, phase: WebPhase): WebPhaseBounds {
  const ends = WEB_PHASE_ENDS[focus];
  const planned = WEB_PHASE_ORDER.filter((candidate) => ends[candidate] !== undefined);
  let start = 0;
  for (let index = 0; index < planned.length; index += 1) {
    const candidate = planned[index];
    const end = ends[candidate] ?? 1;
    if (candidate === phase) {
      const next = planned[index + 1];
      return {
        start,
        end,
        nextEnd: next ? (ends[next] ?? 1) : 1,
        nextHalfLife: next ? WEB_PHASE_HALF_LIFE_MS[next] : HANDOFF_HALF_LIFE_MS,
      };
    }
    if (WEB_PHASE_ORDER.indexOf(candidate) > WEB_PHASE_ORDER.indexOf(phase)) {
      // A phase this focus never runs collapses onto the boundary it would
      // sit at and drifts straight into the phase that follows.
      return { start, end: start, nextEnd: end, nextHalfLife: WEB_PHASE_HALF_LIFE_MS[candidate] };
    }
    start = end;
  }
  return { start, end: start, nextEnd: 1, nextHalfLife: HANDOFF_HALF_LIFE_MS };
}

interface WebMilestone {
  phase: WebPhase;
  /** Fraction of the phase confirmed by the event. */
  fraction: number;
}

function webMilestone(event: ScanProgressEvent): WebMilestone | null {
  if (event.checks_total > 0) {
    return { phase: "checks", fraction: clamp01(event.checks_done / event.checks_total) };
  }
  const finished = event.status !== "running";
  switch (event.check_id) {
    case "fetch":
      // The request is in flight; the first check event is what confirms the fetch.
      return { phase: "fetch", fraction: finished ? 1 : 0.25 };
    case "polish-css":
      return { phase: "polish", fraction: finished ? 0.4 : 0 };
    case "polish-signals":
      return { phase: "polish", fraction: finished ? 1 : 0.4 };
    case "browser-analysis":
      return { phase: "browser", fraction: finished ? 1 : 0 };
    default:
      return null;
  }
}

function isLastPage(tracks: ScanRunTracks): boolean {
  return tracks.pageIndex >= tracks.pageCount - 1;
}

function finishWeb(tracks: ScanRunTracks, nowMs: number): ScanRunTracks {
  if (tracks.webDone) return tracks;
  // The code collector's handoff drift starts the moment the web step ends.
  return { ...tracks, webDone: true, code: freshCodeDrift(nowMs) };
}

function advanceWeb(tracks: ScanRunTracks, event: ScanProgressEvent, nowMs: number): ScanRunTracks {
  const milestone = webMilestone(event);
  if (!milestone || tracks.plan.web === null || tracks.webDone) return tracks;
  const bounds = webPhaseBounds(tracks.plan.web, milestone.phase);
  const floor = bounds.start + (bounds.end - bounds.start) * milestone.fraction;
  const finished = milestone.fraction >= 1 || bounds.end <= bounds.start;
  const web = settle(
    tracks.web,
    floor,
    finished ? bounds.nextEnd : bounds.end,
    finished ? bounds.nextHalfLife : WEB_PHASE_HALF_LIFE_MS[milestone.phase],
    nowMs,
  );
  const next = { ...tracks, web, pageSettled: tracks.pageSettled || web.floor >= 1 };
  return web.floor >= 1 && isLastPage(next) ? finishWeb(next, nowMs) : next;
}

function advanceCode(
  tracks: ScanRunTracks,
  event: ScanProgressEvent,
  nowMs: number,
): ScanRunTracks {
  const stage = event.check_id.slice(CODE_SCAN_PREFIX.length);
  const fraction = clamp01(event.checks_done / Math.max(1, event.checks_total));
  const stageEnd = CODE_STAGE_ENDS[stage];
  const ceiling = stageEnd ?? Math.min(1, fraction + CODE_UNKNOWN_STAGE_HEADROOM);
  const halfLife = CODE_STAGE_HALF_LIFE_MS[stage] ?? CODE_STAGE_DEFAULT_HALF_LIFE_MS;
  // A code event proves the web step is over, whatever its last event said.
  const handedOff = finishWeb(tracks, nowMs);
  const plan = handedOff.plan.code ? handedOff.plan : { ...handedOff.plan, code: true };
  const code = settle(handedOff.code, fraction, ceiling, halfLife, nowMs);
  return {
    ...handedOff,
    plan,
    code,
    codeSeen: true,
    codeDone: handedOff.codeDone || code.floor >= 1,
  };
}

/** Fold one `scan-progress` event into the run. Pure; returns the same object when nothing changes. */
export function advanceScanRunTracks(
  tracks: ScanRunTracks,
  event: ScanProgressEvent,
  nowMs: number,
): ScanRunTracks {
  return event.check_id.startsWith(CODE_SCAN_PREFIX)
    ? advanceCode(tracks, event, nowMs)
    : advanceWeb(tracks, event, nowMs);
}

/** Fold one `multi-scan-progress` event into the run. */
export function advanceScanRunPages(
  tracks: ScanRunTracks,
  event: MultiScanProgressEvent,
  nowMs: number,
): ScanRunTracks {
  if (tracks.webDone) return tracks;
  const pageCount = Math.max(1, event.page_count);
  const pageIndex = Math.min(Math.max(0, event.page_index), pageCount - 1);
  const settled = event.page_status === "complete" || event.page_status === "error";
  const newPage = pageIndex !== tracks.pageIndex || pageCount !== tracks.pageCount;
  const next: ScanRunTracks = {
    ...tracks,
    pageIndex,
    pageCount,
    pageSettled: settled || (newPage ? false : tracks.pageSettled),
    web: newPage && !settled ? freshWebDrift(tracks.plan.web, nowMs) : tracks.web,
  };
  return settled && pageIndex >= pageCount - 1 ? finishWeb(next, nowMs) : next;
}

function toPercent(fraction: number): number {
  return Math.min(100, Math.max(0, 100 * clamp01(fraction)));
}

/**
 * The active step's percent at `nowMs`, before the store's high-water guard.
 * The web step holds at 100 once it finishes until the first code event moves
 * the ring to the Code Scan step; pages split the web step evenly.
 */
export function estimateScanRunProgress(
  tracks: ScanRunTracks,
  nowMs: number,
): ScanRunProgressEstimate {
  const hasWeb = tracks.plan.web !== null;
  const step: ScanRunStepKind = !hasWeb || tracks.codeSeen ? "code" : "web";

  if (step === "web") {
    if (tracks.webDone) return { step, percent: 100 };
    const page = tracks.pageSettled ? 1 : driftValue(tracks.web, nowMs);
    const fraction = tracks.pageCount > 1 ? (tracks.pageIndex + page) / tracks.pageCount : page;
    return { step, percent: toPercent(fraction) };
  }

  if (!tracks.plan.code) return { step, percent: 0 };
  if (tracks.codeDone) return { step, percent: 100 };
  return { step, percent: toPercent(driftValue(tracks.code, nowMs)) };
}
