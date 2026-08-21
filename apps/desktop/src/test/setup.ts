import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// Tauri APIs need browser-global stubs in jsdom. Tests may override them with
// hoisted `vi.mock` calls.
if (typeof window !== "undefined" && !("__TAURI_EVENT_PLUGIN_INTERNALS__" in window)) {
  Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
    configurable: true,
    value: {
      unregisterListener: () => {},
    },
  });
}

if (typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window)) {
  let nextCallbackId = 1;
  const callbacks = new Map<number, (...args: unknown[]) => void>();
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {
      transformCallback: (callback?: (...args: unknown[]) => void) => {
        const id = nextCallbackId++;
        if (callback) callbacks.set(id, callback);
        return id;
      },
      // Tests that need a response override this with `vi.mock`.
      invoke: () => Promise.resolve(null),
      runCallback: (id: number, ...args: unknown[]) => {
        callbacks.get(id)?.(...args);
      },
      unregisterListener: (_event: string, eventId: number) => {
        callbacks.delete(eventId);
        return Promise.resolve();
      },
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { label: "main" },
      },
    },
  });
}

afterEach(() => {
  cleanup();
});
