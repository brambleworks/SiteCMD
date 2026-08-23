// Handles both runtime activation events and cold-start deep-link state.

import { useEffect, useLayoutEffect, useRef } from "react";
import { getCurrent as getCurrentDeepLinks } from "@tauri-apps/plugin-deep-link";

import { confirmLinkLicenseActivation } from "@/lib/commands";
import { useTier } from "@/hooks/useTier";
import { useToast } from "@/hooks/useToast";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { shouldIgnoreRepeatedDeepLink } from "@/app/app-shell-helpers";
import { latestActivateDeepLinkKey } from "@/lib/deep-links";
import { getTierDisplayName } from "@/lib/tier-labels";
import { SUPPORT_EMAIL } from "@/lib/support";
import {
  activationFailureAction,
  formatLicenseActivationError,
} from "@/lib/license-activation-error";
import { userFacingError } from "@/lib/user-facing-error";

/** Deduplicate cold-start URLs that also arrive through the runtime event. */
const SAME_KEY_DEDUPE_WINDOW_MS = 5_000;

export function useLicenseActivateDeepLink() {
  const { activateLicense, refreshLicense } = useTier();
  const { success, error, warning, info } = useToast();
  const lastHandledRef = useRef<{ key: string; handledAt: number } | null>(null);

  const handleActivationRequest = (rawKey: string | undefined) => {
    const key = rawKey?.trim();
    if (!key) {
      error("Couldn't activate license", "Open the link from your purchase email again.");
      return;
    }
    // Match the Account input bound before crossing IPC.
    if (key.length > 256) {
      error(
        "Couldn't activate license",
        `The activation link is malformed. Contact ${SUPPORT_EMAIL}.`,
      );
      return;
    }
    const now = Date.now();
    const lastHandled = lastHandledRef.current;
    if (
      shouldIgnoreRepeatedDeepLink({
        nextKey: key,
        lastKey: lastHandled?.key ?? null,
        elapsedMs: lastHandled ? now - lastHandled.handledAt : Number.POSITIVE_INFINITY,
        dedupeWindowMs: SAME_KEY_DEDUPE_WINDOW_MS,
      })
    ) {
      return;
    }
    lastHandledRef.current = { key, handledAt: now };
    void (async () => {
      try {
        if (!(await confirmLinkLicenseActivation())) {
          // False can mean either a decline or a native dialog failure.
          info(
            "License not activated",
            "Nothing was changed. You can activate this license any time in Settings, then Account.",
          );
          return;
        }
      } catch (e) {
        error(
          "Couldn't activate license",
          `${userFacingError(e, "Try again in a moment.")} You can still activate it in Settings, then Account.`,
        );
        return;
      }
      try {
        const info = await activateLicense(key);
        if (!info.isActive) {
          warning(
            "License checked",
            "The license could not be confirmed as active, so SiteCMD is on the Free plan. Open Settings, then Account, for what to do next.",
          );
          return;
        }
        const tierLabel = getTierDisplayName(info.tier);
        success(`${tierLabel} active`, "License activated. Welcome.");
      } catch (err) {
        const action = activationFailureAction(err);
        if (action.announce === "nothing") {
          return;
        }
        if (action.announce === "pending") {
          warning(
            "Activation could not be confirmed",
            "SiteCMD cannot tell whether this license was installed. Open Settings, then Account: if a key field is showing, enter your key there. If a license is already shown, choose Refresh License to pick up the change. Either is safe.",
          );
          void refreshLicense();
          return;
        }
        error("License activation failed", formatLicenseActivationError(action.payload));
        void refreshLicense();
      }
    })();
  };

  useTauriEvent("sitecmd-license-activate-requested", (payload) =>
    handleActivationRequest(payload?.key),
  );

  // Expose the latest handler to the mount-only cold-start effect.
  const handleRef = useRef(handleActivationRequest);
  useLayoutEffect(() => {
    handleRef.current = handleActivationRequest;
  });

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      let urls: string[] | null;
      try {
        urls = await getCurrentDeepLinks();
      } catch {
        return;
      }
      if (cancelled) return;
      const key = latestActivateDeepLinkKey(urls);
      if (key) handleRef.current(key);
    })();
    return () => {
      cancelled = true;
    };
  }, []);
}
