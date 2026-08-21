import { useEffect, useRef } from "react";
import { getCurrent as getCurrentDeepLinks, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import type { AppTarget } from "@/lib/app-targets";
import { getLatestDeepLinkEnvelope, shouldIgnoreRepeatedDeepLink } from "@/app/app-shell-helpers";

export function useDeepLinkTargets(openAppTarget: (target: AppTarget) => void) {
  const lastHandledDeepLinkRef = useRef<{ key: string; handledAt: number } | null>(null);

  useEffect(() => {
    const openUrls = (urls: string[] | null | undefined) => {
      const envelope = getLatestDeepLinkEnvelope(urls);
      if (!envelope) return;
      const now = Date.now();
      const lastHandled = lastHandledDeepLinkRef.current;
      if (
        shouldIgnoreRepeatedDeepLink({
          nextKey: envelope.dedupeKey,
          lastKey: lastHandled?.key ?? null,
          elapsedMs: lastHandled ? now - lastHandled.handledAt : Number.POSITIVE_INFINITY,
        })
      ) {
        return;
      }
      lastHandledDeepLinkRef.current = {
        key: envelope.dedupeKey,
        handledAt: now,
      };
      openAppTarget(envelope.target);
    };

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        const currentUrls = await getCurrentDeepLinks();
        if (!cancelled) {
          openUrls(currentUrls);
        }
      } catch {
        // Deep links are optional; ignore if unavailable.
      }

      try {
        const stop = await onOpenUrl((urls) => {
          openUrls(urls);
        });
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
      } catch {
        // Listener is best-effort across platforms.
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [openAppTarget]);
}
