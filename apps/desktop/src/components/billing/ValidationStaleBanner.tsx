import { useState } from "react";
import { AlertTriangle, KeyRound, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useTier } from "@/hooks/useTier";
import { useToast } from "@/hooks/useToast";
import { normalizePlanDisplayName } from "@/lib/tier-labels";
import { SUPPORT_EMAIL } from "@/lib/support";

export function ValidationStaleBanner() {
  const { licenseInfo, refreshLicense } = useTier();
  const { error: showErrorToast } = useToast();
  const [isRetrying, setIsRetrying] = useState(false);
  const warning = licenseInfo.validationWarning;
  // Unknown warning codes remain hidden for forward-compatible state reads.
  if (
    warning !== "stale" &&
    warning !== "stale_final_warning" &&
    warning !== "key_unreadable" &&
    warning !== "instance_deactivated"
  ) {
    return null;
  }

  const isFinal = warning === "stale_final_warning";
  const isKeyUnreadable = warning === "key_unreadable";
  // A removed machine activation needs re-entry guidance, not expiry copy.
  const isInstanceDeactivated = warning === "instance_deactivated";
  const Icon =
    isKeyUnreadable || isInstanceDeactivated ? KeyRound : isFinal ? AlertTriangle : WifiOff;
  const className = isFinal ? "validation-stale-banner--final" : "validation-stale-banner";
  const planName = normalizePlanDisplayName(licenseInfo.planName);
  const message = isInstanceDeactivated
    ? "This machine's license activation was removed, so connected service and maintained guide updates are unavailable here. Local scans and saved guidance still work. Re-enter your license key in Settings to reactivate this machine."
    : isKeyUnreadable
      ? `Couldn't read your ${planName} license key from this device's secure storage. Connected service access may be unavailable, but local scans and fixes are unaffected. Re-enter your license key in Settings to fix this.`
      : isFinal
        ? `Couldn't reach the license server for over 7 days. Your ${planName} connected service access and maintained guide updates will pause in 24 hours unless validation succeeds. Local scans and fixes remain available. Check your network or contact ${SUPPORT_EMAIL}.`
        : `Couldn't reach the license server to validate your ${planName} license. Connected service access is using cached validation. Local scans and fixes are unaffected. Check your connection or try again later.`;

  async function handleRetry() {
    setIsRetrying(true);
    // Only a successful refresh clears the warning and unmounts the banner.
    const info = await refreshLicense();
    setIsRetrying(false);
    if (info?.validationWarning === "none") return;
    if (info?.validationWarning === "key_unreadable") {
      showErrorToast("Your license key couldn't be read. Re-enter it in Settings.");
    } else {
      showErrorToast("Still couldn't reach the license server.");
    }
  }

  return (
    <div className={className} role="alert" aria-live="polite">
      <Icon className="icon-md" aria-hidden="true" />
      <span>{message}</span>
      {!isKeyUnreadable && !isInstanceDeactivated && (
        <Button
          variant="outline"
          size="sm"
          className="validation-stale-banner__retry"
          disabled={isRetrying}
          onClick={() => void handleRetry()}>
          {isRetrying ? "Retrying..." : "Retry"}
        </Button>
      )}
    </div>
  );
}
