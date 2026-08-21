import { listen as tauriListen } from "@tauri-apps/api/event";
import type { EventCallback, EventName, UnlistenFn } from "@tauri-apps/api/event";

type TauriWindowGlobals = Window & {
  __TAURI_INTERNALS__?: {
    transformCallback?: unknown;
  };
  __TAURI_EVENT_PLUGIN_INTERNALS__?: {
    unregisterListener?: unknown;
  };
};

const warnedEvents = new Set<string>();

function hasTauriEventBridge(): boolean {
  if (typeof window === "undefined") return false;
  const globals = window as TauriWindowGlobals;
  return (
    typeof globals.__TAURI_INTERNALS__?.transformCallback === "function" &&
    typeof globals.__TAURI_EVENT_PLUGIN_INTERNALS__?.unregisterListener === "function"
  );
}

function warnUnavailable(event: string, error?: unknown) {
  if (!import.meta.env.DEV) return;
  const key = `${event}:${error instanceof Error ? error.message : "missing-bridge"}`;
  if (warnedEvents.has(key)) return;
  warnedEvents.add(key);
  console.warn(
    `[tauri-events] Skipping listener for "${event}" because the Tauri event bridge is unavailable.`,
    error,
  );
}

export async function safeListen<T>(
  event: EventName,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  if (!hasTauriEventBridge()) {
    warnUnavailable(String(event));
    return () => {};
  }

  try {
    const unlisten = await tauriListen<T>(event, handler);
    // StrictMode cleanup requires an idempotent Tauri unlisten wrapper.
    let unlistened = false;
    return () => {
      if (unlistened) return;
      unlistened = true;
      try {
        // UnlistenFn is typed synchronous, but some Tauri builds reject
        // asynchronously; guard both the throw and the rejection paths.
        const result: unknown = (unlisten as () => unknown)();
        if (result instanceof Promise) result.catch(() => {});
      } catch {
        // Listener already gone - safe to ignore.
      }
    };
  } catch (error) {
    warnUnavailable(String(event), error);
    return () => {};
  }
}
