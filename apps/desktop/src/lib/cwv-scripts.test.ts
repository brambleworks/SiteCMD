import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEBVIEW_DIR = path.resolve(HERE, "../../src-tauri/crates/engine/browser");

type PerformanceEntryFixture = {
  startTime: number;
  value?: number;
  duration?: number;
  hadRecentInput?: boolean;
  name?: string;
  responseStart?: number;
};

function runObserverScript() {
  const callbacks = new Map<string, (list: { getEntries(): PerformanceEntryFixture[] }) => void>();
  class PerformanceObserverMock {
    static supportedEntryTypes = ["layout-shift", "longtask"];
    private readonly callback: (list: { getEntries(): PerformanceEntryFixture[] }) => void;

    constructor(callback: (list: { getEntries(): PerformanceEntryFixture[] }) => void) {
      this.callback = callback;
    }

    observe(options: { type: string }) {
      callbacks.set(options.type, this.callback);
    }
  }

  const windowFixture: Record<string, unknown> & {
    addEventListener(): void;
    __SHK_CWV__?: {
      cls: number | null;
      observed_long_task_blocking_ms?: number | null;
      tbt_ms?: number | null;
    };
  } = {
    addEventListener() {},
  };
  const performanceFixture = {
    getEntriesByType: () => [],
    timing: null,
  };
  const script = readFileSync(path.join(WEBVIEW_DIR, "cwv_observer.js"), "utf8");
  new Function("window", "performance", "PerformanceObserver", script)(
    windowFixture,
    performanceFixture,
    PerformanceObserverMock,
  );

  return { callbacks, cwv: windowFixture.__SHK_CWV__! };
}

function runReadScript(entries: Record<string, PerformanceEntryFixture[]>) {
  class PerformanceObserverMock {
    static supportedEntryTypes = ["layout-shift", "longtask"];
  }
  const windowFixture: Record<string, unknown> & {
    __SHK_CWV__?: {
      cls: number | null;
      observed_long_task_blocking_ms?: number | null;
      tbt_ms?: number | null;
    };
  } = {};
  const performanceFixture = {
    getEntriesByType: (type: string) => entries[type] ?? [],
  };
  const documentFixture = { title: "" };
  const script = readFileSync(path.join(WEBVIEW_DIR, "cwv_read.js"), "utf8");
  new Function("window", "performance", "PerformanceObserver", "document", script)(
    windowFixture,
    performanceFixture,
    PerformanceObserverMock,
    documentFixture,
  );
  return windowFixture.__SHK_CWV__!;
}

describe("webview performance scripts", () => {
  it("reports the largest CLS session window instead of summing separated bursts", () => {
    const { callbacks, cwv } = runObserverScript();
    callbacks.get("layout-shift")!({
      getEntries: () => [
        { startTime: 100, value: 0.1, hadRecentInput: false },
        { startTime: 700, value: 0.05, hadRecentInput: false },
        { startTime: 7_000, value: 0.2, hadRecentInput: false },
      ],
    });

    expect(cwv.cls).toBeCloseTo(0.2);
  });

  it("uses the same CLS session-window definition in the fallback reader", () => {
    const cwv = runReadScript({
      "layout-shift": [
        { startTime: 100, value: 0.1, hadRecentInput: false },
        { startTime: 700, value: 0.05, hadRecentInput: false },
        { startTime: 7_000, value: 0.2, hadRecentInput: false },
      ],
    });

    expect(cwv.cls).toBeCloseTo(0.2);
  });

  it("reports observed post-FCP blocking without calling the sample TBT", () => {
    const { callbacks, cwv } = runObserverScript();
    callbacks.get("paint")!({
      getEntries: () => [{ name: "first-contentful-paint", startTime: 1_000 }],
    });
    callbacks.get("longtask")!({
      getEntries: () => [
        { startTime: 500, duration: 200 },
        // The task begins before FCP, but its blocking portion continues for
        // 50 ms after FCP and must not disappear from the observation.
        { startTime: 900, duration: 150 },
        { startTime: 1_200, duration: 100 },
      ],
    });

    expect(cwv.observed_long_task_blocking_ms).toBe(100);
    expect(cwv).not.toHaveProperty("tbt_ms");
  });

  it("uses the same post-FCP blocking definition in the fallback reader", () => {
    const cwv = runReadScript({
      paint: [{ name: "first-contentful-paint", startTime: 1_000 }],
      longtask: [
        { startTime: 500, duration: 200 },
        { startTime: 900, duration: 150 },
        { startTime: 1_200, duration: 100 },
      ],
    });

    expect(cwv.observed_long_task_blocking_ms).toBe(100);
    expect(cwv).not.toHaveProperty("tbt_ms");
  });
});
