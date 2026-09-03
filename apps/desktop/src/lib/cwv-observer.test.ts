import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const BROWSER_DIR = path.resolve(HERE, "../../src-tauri/crates/engine/browser");
const OBSERVER = readFileSync(path.join(BROWSER_DIR, "cwv_observer.js"), "utf8");
const READER = readFileSync(path.join(BROWSER_DIR, "cwv_read.js"), "utf8");

type Vitals = {
  js_errors: string[];
  js_error_count: number;
  document_url?: string | null;
};

// The page's window: a jsdom event target the observer installs its
// listeners on, with the location the readback records. One per test, so
// listeners never pile up across tests.
type Page = EventTarget & { location: Location; __SHK_CWV__?: Vitals };

// What Tauri's ACL rejects with when the notification plugin's init script
// invokes its command inside the analyzer webview.
const ACL_REJECTION =
  'notification.is_permission_granted not allowed on window "analyzer-1788391062761", webview "analyzer-1788391062761", URL: https://example.com/\n\nreferenced by: capability: default';

// The analyzer injects the observer at document start, before any page script.
function installObserver() {
  const page = Object.assign(new EventTarget(), { location: window.location }) as Page;
  new Function("window", "performance", "PerformanceObserver", OBSERVER)(
    page,
    window.performance,
    (globalThis as { PerformanceObserver?: unknown }).PerformanceObserver,
  );
  return { page, vitals: page.__SHK_CWV__! };
}

function readback(page: Page) {
  new Function("window", "performance", "PerformanceObserver", "document", READER)(
    page,
    window.performance,
    (globalThis as { PerformanceObserver?: unknown }).PerformanceObserver,
    window.document,
  );
}

function reject(page: Page, reason: unknown) {
  page.dispatchEvent(Object.assign(new Event("unhandledrejection"), { reason }));
}

function pageError(page: Page, message: string, filename = "https://example.com/app.js") {
  page.dispatchEvent(new ErrorEvent("error", { message, filename, lineno: 12 }));
}

describe("analyzer Core Web Vitals observer", () => {
  it("does not count the analyzer runtime's own rejection as a page error", () => {
    const { page, vitals } = installObserver();

    reject(page, new Error(ACL_REJECTION));
    reject(page, ACL_REJECTION);
    reject(page, new TypeError("window.__TAURI_INTERNALS__.invoke is not a function"));
    reject(page, "plugin:notification|is_permission_granted failed");

    expect(vitals.js_error_count).toBe(0);
    expect(vitals.js_errors).toEqual([]);
  });

  it("still counts the page's own errors and rejections", () => {
    const { page, vitals } = installObserver();

    pageError(page, "TypeError: undefined is not an object (evaluating 'window.dataLayer.push')");
    reject(page, new Error("Failed to fetch"));

    expect(vitals.js_error_count).toBe(2);
    expect(vitals.js_errors).toEqual([
      "TypeError: undefined is not an object (evaluating 'window.dataLayer.push') (https://example.com/app.js:12)",
      "Unhandled promise rejection: Failed to fetch",
    ]);
  });

  it("counts a build-tool overlay error whose text starts with plugin:", () => {
    // SiteCMD scans local dev servers. A Vite or Rollup overlay error reads
    // "[plugin:vite:import-analysis] ...", and dropping it would report a
    // clean page the run never observed.
    const { page, vitals } = installObserver();

    pageError(page, '[plugin:vite:import-analysis] Failed to resolve import "./missing"');
    reject(page, new Error("rollup plugin: terser failed"));

    expect(vitals.js_error_count).toBe(2);
    expect(vitals.js_errors[0]).toContain("plugin:vite:import-analysis");
  });

  it("still drops a Tauri command rejection, which always carries a pipe", () => {
    const { page, vitals } = installObserver();

    reject(page, new Error("plugin:window-state|restore_state failed"));

    expect(vitals.js_error_count).toBe(0);
  });

  it("keeps counting page errors after a runtime rejection", () => {
    const { page, vitals } = installObserver();

    reject(page, new Error(ACL_REJECTION));
    pageError(page, "ReferenceError: gtag is not defined");

    expect(vitals.js_error_count).toBe(1);
    expect(vitals.js_errors).toEqual([
      "ReferenceError: gtag is not defined (https://example.com/app.js:12)",
    ]);
  });
});

describe("analyzer Core Web Vitals readback", () => {
  // The adapter refuses a sample that does not name the document it came
  // from, so the reader records the page's location with the metrics.
  it("records the document the sample was read from", () => {
    const { page, vitals } = installObserver();
    readback(page);

    expect(vitals.document_url).toBe(window.location.href);
    expect(vitals.js_error_count).toBe(0);
  });

  it("leaves the document unidentified when the page has no location", () => {
    const { page, vitals } = installObserver();
    delete (page as Partial<Page>).location;
    readback(page);

    expect(vitals.document_url).toBeNull();
  });
});
