import { useEffect, useLayoutEffect, useRef } from "react";

import type { AppEventName, AppEventPayloads } from "@/lib/app-events";
import { safeListen } from "@/lib/tauri-events";

interface UseTauriEventOptions {
  /** Subscribe only while true. Defaults to true. */
  enabled?: boolean;
}

/** Subscribe without stale handlers or late-listener leaks. */
export function useTauriEvent<K extends AppEventName>(
  event: K,
  handler: (payload: AppEventPayloads[K]) => void | Promise<void>,
  options?: UseTauriEventOptions,
): void {
  const handlerRef = useRef(handler);
  useLayoutEffect(() => {
    handlerRef.current = handler;
  });

  const enabled = options?.enabled ?? true;
  useEffect(() => {
    if (!enabled) return;

    let cancelled = false;
    let unlisten: (() => void) | null = null;

    // The callback returns the handler's result so an async handler's promise
    // propagates (Tauri ignores it in production; tests can await it).
    void safeListen<AppEventPayloads[K]>(event, (payloadEvent) =>
      handlerRef.current(payloadEvent.payload),
    ).then((stop) => {
      // Unmounted before the listener attached: detach immediately instead of
      // leaking it. Otherwise hold it for the cleanup below.
      if (cancelled) stop();
      else unlisten = stop;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [event, enabled]);
}
