import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
// The Rust side strips the statement terminator the same way before wrapping
// the function with its arguments.
const SOURCE = readFileSync(
  path.resolve(HERE, "../../src-tauri/src/webview/title_bridge.js"),
  "utf8",
)
  .trim()
  .replace(/;$/, "");

// WebKit truncates every document title to this many UTF-16 units before the
// UI process sees it, and collapses whitespace runs on the way.
const WEBKIT_TITLE_CAP = 1000;
const CHUNK = 900;
const MARKER = "___SHK_AXE___";

type Page = { window: Record<string, unknown>; document: { title: string } };

function page(globals: Record<string, unknown> = {}): Page {
  return { window: { ...globals }, document: { title: "Example Domain" } };
}

function request(target: Page, globalName: string, index: number, marker = MARKER) {
  new Function(
    "window",
    "document",
    `(${SOURCE})(${JSON.stringify(globalName)}, ${JSON.stringify(marker)}, ${index}, ${CHUNK});`,
  )(target.window, target.document);
}

function frame(title: string, marker = MARKER) {
  expect(title.startsWith(marker)).toBe(true);
  const body = title.slice(marker.length);
  const match = /^(\d+)\/(\d+):([A-Za-z0-9+/=]*)$/.exec(body);
  if (!match) throw new Error(`not a frame: ${title}`);
  return { index: Number(match[1]), total: Number(match[2]), data: match[3] };
}

function transfer(target: Page, globalName: string, marker = MARKER) {
  const titles: string[] = [];
  let encoded = "";
  let index = 0;
  let total = 1;
  while (index < total) {
    request(target, globalName, index, marker);
    const current = frame(target.document.title, marker);
    expect(current.index).toBe(index);
    titles.push(target.document.title);
    total = current.total;
    encoded += current.data;
    index += 1;
  }
  return { titles, json: Buffer.from(encoded, "base64").toString("utf8") };
}

const PAYLOAD = {
  violations: [
    {
      id: "color-contrast",
      html: '<p class="lead   intro">Café   crème\n\tñ 🎉 <a href="\\\\share">x</a></p>',
      failure_summary: "Fix any of the following:\n  Element has insufficient color contrast",
    },
  ],
  passes: Array.from({ length: 120 }, (_, i) => `rule-${i}`),
  incomplete: [],
  inapplicable: ["frame-title"],
};

describe("analyzer title bridge chunk script", () => {
  it("answers pending while the global holds nothing", () => {
    const target = page();
    request(target, "__SHK_AXE__", 0);
    expect(target.document.title).toBe(`${MARKER}pending`);

    target.window.__SHK_AXE__ = null;
    request(target, "__SHK_AXE__", 0);
    expect(target.document.title).toBe(`${MARKER}pending`);
  });

  it("frames the JSON as base64 chunks that survive the title channel", () => {
    const target = page({ __SHK_AXE__: PAYLOAD });
    const { titles, json } = transfer(target, "__SHK_AXE__");

    expect(titles.length).toBeGreaterThan(1);
    for (const title of titles) {
      expect(title.length).toBeLessThanOrEqual(WEBKIT_TITLE_CAP);
      // Whitespace would be collapsed or stripped by the webview, and a
      // backslash rewritten under some page encodings; base64 has neither.
      expect(title).not.toMatch(/[\s\\]/);
    }
    expect(JSON.parse(json)).toEqual(PAYLOAD);
    expect(json).toBe(JSON.stringify(PAYLOAD));
  });

  it("encodes once so a value that keeps changing still transfers whole", () => {
    const vitals: Record<string, unknown> = { lcp_ms: 1200, cls: 0.01, js_errors: [] };
    const target = page({ __SHK_CWV__: vitals });
    request(target, "__SHK_CWV__", 0, "___SHK_CWV___");
    const first = target.document.title;

    // The observer keeps mutating the live object after the first chunk.
    vitals.cls = 0.5;
    (vitals.js_errors as string[]).push("late error");
    request(target, "__SHK_CWV__", 0, "___SHK_CWV___");
    expect(target.document.title).toBe(first);

    const { json } = transfer(target, "__SHK_CWV__", "___SHK_CWV___");
    expect(JSON.parse(json)).toEqual({ lcp_ms: 1200, cls: 0.01, js_errors: [] });
  });

  it("keeps one encoding per global", () => {
    const target = page({ __SHK_CWV__: { lcp_ms: 5 }, __SHK_AXE__: { violations: [] } });
    expect(JSON.parse(transfer(target, "__SHK_CWV__", "___SHK_CWV___").json)).toEqual({
      lcp_ms: 5,
    });
    expect(JSON.parse(transfer(target, "__SHK_AXE__").json)).toEqual({ violations: [] });
  });

  it("serves a single chunk for a small value", () => {
    const target = page({ __SHK_CWV__: { lcp_ms: 5 } });
    const { titles } = transfer(target, "__SHK_CWV__", "___SHK_CWV___");
    expect(titles).toHaveLength(1);
    expect(frame(titles[0], "___SHK_CWV___").total).toBe(1);
  });
});
