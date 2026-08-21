import { useEffect, useEffectEvent, useRef } from "react";

interface UseVisibilityRefreshOptions {
  /** Trigger `onRefresh` if the document was hidden for at least this long. */
  staleAfterMs: number;
  /** Called when the document becomes visible after being hidden long enough. */
  onRefresh: () => void;
  /** When false, the listener is detached and no refreshes fire. */
  enabled?: boolean;
}

/** Refresh after the document has remained hidden past `staleAfterMs`. */
export function useVisibilityRefresh({
  staleAfterMs,
  onRefresh,
  enabled = true,
}: UseVisibilityRefreshOptions): void {
  const hiddenSinceRef = useRef<number | null>(null);
  // useEffectEvent keeps the callback current without rebinding the listener.
  const refresh = useEffectEvent(() => onRefresh());

  useEffect(() => {
    if (!enabled || typeof document === "undefined") return;

    if (document.visibilityState === "hidden") {
      hiddenSinceRef.current = Date.now();
    }

    const handleVisibilityChange = () => {
      if (document.visibilityState === "hidden") {
        hiddenSinceRef.current = Date.now();
        return;
      }
      if (hiddenSinceRef.current == null) return;
      const hiddenForMs = Date.now() - hiddenSinceRef.current;
      hiddenSinceRef.current = null;
      if (hiddenForMs >= staleAfterMs) {
        refresh();
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [enabled, staleAfterMs]);
}
