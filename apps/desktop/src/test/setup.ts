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

// jsdom 30 has no modal dialog implementation; mirror the open attribute and the
// real browser's initial-focus behavior so components can rely on showModal() and
// close() in tests, including Escape reaching the dialog's own keydown handler.
if (typeof HTMLDialogElement !== "undefined" && !HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
    this.setAttribute("open", "");
    const focusable = this.querySelector<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );
    (focusable ?? this).focus();
  };
  HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
}

afterEach(() => {
  cleanup();
});
