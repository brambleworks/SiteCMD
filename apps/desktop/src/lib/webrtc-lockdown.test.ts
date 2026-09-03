import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SCRIPT = readFileSync(
  path.resolve(HERE, "../../src-tauri/src/webview/webrtc_lockdown.js"),
  "utf8",
);

// The analyzer injects this at document start in every frame. Running the
// script with `globalThis` shadowed by a plain object checks the removal
// semantics under ordinary JavaScript rules, which is what a real webview
// global provides; Node's vm sandbox intercepts descriptors and prototypes
// differently, so it is used only to prove the file runs as a standalone
// script.
function runOn(target: object) {
  new Function("globalThis", SCRIPT)(target);
}

describe("analyzer WebRTC lockdown script", () => {
  it("runs as a standalone script against a fresh global", () => {
    const context = vm.createContext({ RTCPeerConnection: class {}, fetch: () => undefined });
    vm.runInContext(SCRIPT, context);
    expect("RTCPeerConnection" in context).toBe(false);
    expect(typeof context.fetch).toBe("function");
  });

  it("removes the peer-connection and WebTransport constructors as own properties", () => {
    const target: Record<string, unknown> = {
      RTCPeerConnection: class {},
      webkitRTCPeerConnection: class {},
      WebTransport: class {},
      fetch: () => undefined,
    };
    runOn(target);
    expect("RTCPeerConnection" in target).toBe(false);
    expect("webkitRTCPeerConnection" in target).toBe(false);
    expect("WebTransport" in target).toBe(false);
    // Only the out-of-loader interfaces go; the loader-bound API stays.
    expect(typeof target.fetch).toBe("function");
  });

  it("removes an interface object that lives on the global prototype chain", () => {
    const proto = { RTCPeerConnection: class {} };
    const target = Object.create(proto) as Record<string, unknown>;
    runOn(target);
    expect("RTCPeerConnection" in target).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(proto, "RTCPeerConnection")).toBe(false);
  });

  it("pins a non-configurable interface to undefined instead of leaving it callable", () => {
    const target: Record<string, unknown> = {};
    Object.defineProperty(target, "RTCPeerConnection", {
      value: class {},
      writable: true,
      configurable: false,
    });
    runOn(target);
    expect(target.RTCPeerConnection).toBeUndefined();
    expect(() => {
      target.RTCPeerConnection = class {};
    }).toThrow(TypeError);
  });

  it("is idempotent and tolerates a page without the interfaces", () => {
    const target: Record<string, unknown> = {};
    runOn(target);
    expect(() => runOn(target)).not.toThrow();
    expect(Object.keys(target)).toEqual([]);
  });
});
