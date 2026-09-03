import { describe, expect, it } from "vitest";
import type { MultiScanProgressEvent, ScanProgressEvent } from "@/hooks/useScan";
import type { ScanCategory } from "@/lib/types";
import {
  advanceScanRunPages,
  advanceScanRunTracks,
  createScanRunTracks,
  estimateScanRunProgress,
  type ScanRunPlan,
  type ScanRunTracks,
} from "./scan-progress-model";

function ev(
  atMs: number,
  checkId: string,
  status: ScanProgressEvent["status"],
  done: number,
  total: number,
): { atMs: number; event: ScanProgressEvent } {
  const category = (checkId.split(/[.-]/)[0] || "config") as ScanCategory;
  return {
    atMs,
    event: {
      check_id: checkId,
      category,
      status,
      results_count: 0,
      checks_done: done,
      checks_total: total,
    },
  };
}

function page(
  index: number,
  count: number,
  status: MultiScanProgressEvent["page_status"],
): MultiScanProgressEvent {
  return {
    page_index: index,
    page_count: count,
    current_url: `https://example.com/${index}`,
    page_status: status,
    session_id: 1,
  };
}

/**
 * The renderer-side view of a real smarthomeu.com Web Scan recorded from the
 * CLI on 2026-09-02, after the 100 ms publish throttle collapsed the bursts.
 * Ninety in-memory checks land in the first half second, the origin checks
 * then wait on the network (1.1 s here, 13 s in a sibling run), and the
 * "127 of 127" event never reaches the renderer because the polish event
 * overwrote it inside the same throttle window.
 */
const RECORDED_WEB_RUN = [
  ev(0, "fetch", "running", 0, 0),
  ev(100, "seo.viewport", "complete", 19, 127),
  ev(200, "seo.thin_content", "complete", 32, 127),
  ev(300, "performance.preconnect", "complete", 49, 127),
  ev(400, "accessibility.tabindex", "complete", 64, 127),
  ev(500, "config.www_redirect", "running", 90, 127),
  ev(1615, "config.sitemap_in_robots", "running", 90, 127),
  ev(1715, "polish-css", "complete", 0, 0),
  ev(2003, "polish-signals", "running", 0, 0),
  ev(2103, "polish-signals", "complete", 0, 0),
  ev(2120, "browser-analysis", "running", 0, 0),
  ev(3520, "browser-analysis", "complete", 0, 0),
];

interface Sample {
  atMs: number;
  percent: number;
  /** Percent immediately before the event at this instant was applied, if any. */
  before?: number;
}

/** Replay events through the model, sampling every `stepMs` and around each event. */
function replay(
  plan: ScanRunPlan,
  events: Array<{ atMs: number; event: ScanProgressEvent }>,
  options: {
    stepMs?: number;
    tailMs?: number;
    stretchGapAfterMs?: number;
    stretchBy?: number;
  } = {},
): { samples: Sample[]; tracks: ScanRunTracks } {
  const stepMs = options.stepMs ?? 100;
  const stretched = events.map((entry) =>
    options.stretchGapAfterMs !== undefined && entry.atMs > options.stretchGapAfterMs
      ? { ...entry, atMs: entry.atMs + (options.stretchBy ?? 0) }
      : entry,
  );
  let tracks = createScanRunTracks(plan, 0);
  const samples: Sample[] = [];
  const end = (stretched[stretched.length - 1]?.atMs ?? 0) + (options.tailMs ?? 500);
  let index = 0;
  for (let now = 0; now <= end; now += stepMs) {
    let before: number | undefined;
    while (index < stretched.length && stretched[index].atMs <= now) {
      before = before ?? percentAt(tracks, now);
      tracks = advanceScanRunTracks(tracks, stretched[index].event, now);
      index += 1;
    }
    samples.push({ atMs: now, percent: percentAt(tracks, now), before });
  }
  return { samples, tracks };
}

function percentAt(tracks: ScanRunTracks, nowMs: number): number {
  return estimateScanRunProgress(tracks, nowMs).percent;
}

function assertMonotonic(samples: Sample[]) {
  for (let index = 1; index < samples.length; index += 1) {
    expect(samples[index].percent).toBeGreaterThanOrEqual(samples[index - 1].percent);
  }
}

describe("scan progress model", () => {
  it("keeps moving through an event-free wait without passing the phase end", () => {
    const { samples } = replay(
      { web: "health", code: false },
      RECORDED_WEB_RUN,
      // Stretch the origin-check wait to the 13 s seen in the slow recording.
      { stretchGapAfterMs: 600, stretchBy: 12_000 },
    );
    const waiting = samples.filter((sample) => sample.atMs >= 600 && sample.atMs < 13_600);
    assertMonotonic(waiting);
    for (let index = 1; index < waiting.length; index += 1) {
      expect(waiting[index].percent).toBeGreaterThan(waiting[index - 1].percent);
    }
    expect(waiting[waiting.length - 1].percent).toBeLessThan(60);
    expect(waiting[waiting.length - 1].percent).toBeGreaterThan(55);
  });

  it("never moves backward and lands on 100 for the recorded web run", () => {
    const { samples, tracks } = replay({ web: "health", code: false }, RECORDED_WEB_RUN);
    assertMonotonic(samples);
    expect(samples[samples.length - 1].percent).toBe(100);
    expect(tracks.webDone).toBe(true);
  });

  it("bounds the leap any single event can cause", () => {
    const { samples } = replay({ web: "health", code: false }, RECORDED_WEB_RUN, {
      stretchGapAfterMs: 600,
      stretchBy: 12_000,
    });
    const leaps = samples
      .filter((sample) => sample.before !== undefined)
      .map((sample) => sample.percent - (sample.before ?? 0));
    expect(Math.max(...leaps)).toBeLessThanOrEqual(20);
  });

  it("gives the browser pass the last third and keeps it drifting", () => {
    let tracks = createScanRunTracks({ web: "health", code: false }, 0);
    tracks = advanceScanRunTracks(tracks, ev(0, "polish-signals", "complete", 0, 0).event, 0);
    tracks = advanceScanRunTracks(tracks, ev(0, "browser-analysis", "running", 0, 0).event, 0);
    expect(percentAt(tracks, 0)).toBeCloseTo(68, 5);
    expect(percentAt(tracks, 1_000)).toBeCloseTo(84, 0);
    expect(percentAt(tracks, 11_000)).toBeGreaterThan(95);
    expect(percentAt(tracks, 11_000)).toBeLessThan(100);
    tracks = advanceScanRunTracks(
      tracks,
      ev(0, "browser-analysis", "complete", 0, 0).event,
      11_500,
    );
    expect(percentAt(tracks, 11_500)).toBe(100);
  });

  it("banks drifted progress so a lower late event cannot pull the estimate back", () => {
    let tracks = createScanRunTracks({ web: "health", code: false }, 0);
    tracks = advanceScanRunTracks(tracks, ev(0, "x", "running", 90, 127).event, 0);
    const drifted = percentAt(tracks, 5_000);
    tracks = advanceScanRunTracks(tracks, ev(0, "x", "complete", 91, 127).event, 5_000);
    expect(percentAt(tracks, 5_000)).toBeGreaterThanOrEqual(drifted);
  });

  it("drifts across a phase the pipeline skipped instead of freezing", () => {
    // Local sites skip the browser pass, so polish is the last event.
    let tracks = createScanRunTracks({ web: "health", code: false }, 0);
    tracks = advanceScanRunTracks(tracks, ev(0, "polish-signals", "complete", 0, 0).event, 0);
    expect(percentAt(tracks, 0)).toBeCloseTo(68, 5);
    expect(percentAt(tracks, 3_000)).toBeGreaterThan(85);
    expect(percentAt(tracks, 3_000)).toBeLessThan(100);
  });

  it("ends a security scan with its checks because it runs no polish or browser pass", () => {
    let tracks = createScanRunTracks({ web: "security", code: false }, 0);
    tracks = advanceScanRunTracks(tracks, ev(0, "security.headers", "complete", 5, 10).event, 0);
    expect(percentAt(tracks, 0)).toBeCloseTo(60, 5);
    tracks = advanceScanRunTracks(tracks, ev(0, "security.tls", "complete", 10, 10).event, 100);
    expect(percentAt(tracks, 100)).toBe(100);
  });

  it("fills the web ring to 100, then starts the code ring for the next step of a full scan", () => {
    const plan: ScanRunPlan = { web: "health", code: true };
    let tracks = createScanRunTracks(plan, 0);
    for (const entry of RECORDED_WEB_RUN) {
      tracks = advanceScanRunTracks(tracks, entry.event, entry.atMs);
    }
    expect(estimateScanRunProgress(tracks, 3_520)).toEqual({ step: "web", percent: 100 });
    // The web ring holds at 100 while the web scan persists.
    expect(estimateScanRunProgress(tracks, 4_000)).toEqual({ step: "web", percent: 100 });

    tracks = advanceScanRunTracks(
      tracks,
      ev(0, "code-scan.collect-files", "running", 5, 100).event,
      4_100,
    );
    const codeStart = estimateScanRunProgress(tracks, 4_100);
    expect(codeStart.step).toBe("code");
    expect(codeStart.percent).toBeGreaterThanOrEqual(5);
    expect(codeStart.percent).toBeLessThan(10);
    tracks = advanceScanRunTracks(
      tracks,
      ev(0, "code-scan.analyze-source", "running", 15, 100).event,
      4_300,
    );
    const analyzing = percentAt(tracks, 4_300);
    expect(analyzing).toBeCloseTo(15, 5);
    // Analyze-source drifts toward its own stage end, never past it.
    expect(percentAt(tracks, 30_000)).toBeLessThan(55);
    expect(percentAt(tracks, 30_000)).toBeGreaterThan(analyzing);

    tracks = advanceScanRunTracks(
      tracks,
      ev(0, "code-scan.complete", "complete", 100, 100).event,
      31_000,
    );
    expect(estimateScanRunProgress(tracks, 31_000)).toEqual({ step: "code", percent: 100 });
  });

  it("treats a code event as proof the web step finished, even without a browser event", () => {
    let tracks = createScanRunTracks({ web: "health", code: true }, 0);
    tracks = advanceScanRunTracks(tracks, ev(0, "x", "running", 40, 127).event, 0);
    tracks = advanceScanRunTracks(
      tracks,
      ev(0, "code-scan.collect-files", "running", 5, 100).event,
      2_000,
    );
    expect(tracks.webDone).toBe(true);
    expect(estimateScanRunProgress(tracks, 2_000)).toEqual({ step: "code", percent: 5 });
  });

  it("adopts a code step it was not planned for instead of ignoring its events", () => {
    let tracks = createScanRunTracks({ web: null, code: false }, 0);
    tracks = advanceScanRunTracks(
      tracks,
      ev(0, "code-scan.analyze-source", "running", 42, 100).event,
      0,
    );
    expect(tracks.plan.code).toBe(true);
    expect(estimateScanRunProgress(tracks, 0)).toEqual({ step: "code", percent: 42 });
  });

  it("maps pages onto one climbing number for a multi-page session", () => {
    const plan: ScanRunPlan = { web: "health", code: false, pageCount: 2 };
    let tracks = createScanRunTracks(plan, 0);
    tracks = advanceScanRunPages(tracks, page(0, 2, "scanning"), 0);
    tracks = advanceScanRunTracks(tracks, ev(0, "browser-analysis", "complete", 0, 0).event, 3_000);
    expect(percentAt(tracks, 3_000)).toBe(50);
    tracks = advanceScanRunPages(tracks, page(0, 2, "complete"), 3_050);
    expect(percentAt(tracks, 3_050)).toBe(50);

    tracks = advanceScanRunPages(tracks, page(1, 2, "scanning"), 3_100);
    tracks = advanceScanRunTracks(tracks, ev(0, "fetch", "running", 0, 0).event, 3_120);
    const secondPageStart = percentAt(tracks, 3_120);
    expect(secondPageStart).toBeGreaterThanOrEqual(50);
    expect(secondPageStart).toBeLessThan(52);
    tracks = advanceScanRunTracks(tracks, ev(0, "x", "complete", 64, 127).event, 3_500);
    expect(percentAt(tracks, 3_500)).toBeCloseTo(50 + 50 * 0.352, 0);

    tracks = advanceScanRunPages(tracks, page(1, 2, "error"), 6_000);
    expect(percentAt(tracks, 6_000)).toBe(100);
    expect(tracks.webDone).toBe(true);
  });

  it("starts a run at zero and drifts into the first phase before any event", () => {
    const tracks = createScanRunTracks({ web: "health", code: false }, 0);
    expect(percentAt(tracks, 0)).toBe(0);
    expect(percentAt(tracks, 1_500)).toBeCloseTo(5, 5);
    expect(percentAt(tracks, 60_000)).toBeLessThan(10);
  });

  it("reports nothing for an empty plan", () => {
    const tracks = createScanRunTracks({ web: null, code: false }, 0);
    expect(percentAt(tracks, 5_000)).toBe(0);
  });

  it("starts a code-only run on the code ring and drifts into its first stage", () => {
    const tracks = createScanRunTracks({ web: null, code: true }, 0);
    expect(estimateScanRunProgress(tracks, 0)).toEqual({ step: "code", percent: 0 });
    expect(estimateScanRunProgress(tracks, 1_500)).toEqual({ step: "code", percent: 2.5 });
  });
});
