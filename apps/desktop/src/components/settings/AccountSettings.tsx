import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { getCatalogStatus, retryCatalogRefresh } from "@/lib/commands";
import { errorMessage } from "@/lib/error-message";
import { DEACTIVATION_KEYCHAIN_REMNANT } from "@/lib/license-deactivation";
import { isPrivilegedCommandTimeoutError } from "@/lib/privileged-command-bridge";
import { queryKeys } from "@/lib/query/query-keys";
import { useTier, type BillingInterval, type Tier } from "@/hooks/useTier";
import { useToast } from "@/hooks/useToast";
import { Key, Check, Loader2, Zap } from "lucide-react";
import { Button } from "@/components/ui/button";
import { openUrl } from "@/lib/open-url";
import { SUPPORT_EMAIL } from "@/lib/support";
import {
  activationFailureAction,
  formatLicenseActivationError,
  parseLicenseActivationError,
} from "@/lib/license-activation-error";
import { getTierBadgeLabel, normalizePlanDisplayName } from "@/lib/tier-labels";
import { userFacingError } from "@/lib/user-facing-error";

const BILLING_INTERVAL_LABEL: Record<BillingInterval, string> = {
  monthly: "MONTHLY",
  yearly: "YEARLY",
};

function currentPlanLabel(tier: Tier, billingInterval: BillingInterval | null) {
  const tierLabel = getTierBadgeLabel(tier);
  if (tier === "free" || !billingInterval) return tierLabel;
  return `${tierLabel} ${BILLING_INTERVAL_LABEL[billingInterval]}`;
}

const FOUNDER_BETA_FEATURES = [
  "Watches every production deploy and rescans the live site minutes later",
  "Scheduled hosted scans between deploys, even while your laptop is closed",
  "Alerts only on new findings or regressions, by email or webhook",
  "Verified-fixed tracking against a shared hosted baseline",
  "CI gate that fails a pull request only on new findings",
  "Hosted report delivery",
  "Maintained check and advisory updates, delivered continuously",
];

const FOUNDER_BETA_CONTACT_URL = "https://sitecmd.com/contact";

export function AccountSection() {
  const { tier, licenseInfo, activateLicense, deactivateLicense, refreshLicense } = useTier();
  const catalogStatus = useQuery({
    enabled: licenseInfo.isActive,
    queryFn: getCatalogStatus,
    queryKey: queryKeys.settings.catalogStatus(),
  }).data;
  const toast = useToast();
  const [licenseKey, setLicenseKey] = useState("");
  const [activating, setActivating] = useState(false);
  const [deactivating, setDeactivating] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [showDeactivateConfirm, setShowDeactivateConfirm] = useState(false);
  const [showPlans, setShowPlans] = useState(false);
  const plansExpanded = !licenseInfo.isActive || showPlans;

  const handleActivate = async () => {
    const key = licenseKey.trim();
    if (!key) return;
    setActivating(true);
    try {
      const info = await activateLicense(key);
      if (info.isActive) {
        toast.success(
          "License activated",
          `Welcome to SiteCMD ${normalizePlanDisplayName(info.planName)}!`,
        );
      } else {
        toast.warning(
          "License checked",
          "The license could not be confirmed as active, so SiteCMD is on the Free plan. If this machine was offline, reconnect and enter the key again. If its activation was removed elsewhere, entering the key again re-activates it.",
        );
      }
      setLicenseKey("");
    } catch (e) {
      const action = activationFailureAction(e);
      if (action.announce === "nothing") {
        return;
      }
      if (action.announce === "pending") {
        // Refresh display state without inferring this attempt's outcome.
        toast.warning(
          "Activation could not be confirmed",
          "SiteCMD cannot tell whether this key was installed. This panel updates on its own if it was. If it still shows the key field in a moment, enter the key again - that is safe either way.",
        );
        void refreshLicense();
        return;
      }
      toast.error("Activation failed", formatLicenseActivationError(action.payload));
      // A timed-out database write may still change the stored license state.
      void refreshLicense();
    } finally {
      setActivating(false);
    }
  };

  const handleRefreshLicense = async () => {
    setRefreshing(true);
    try {
      // Keep the button locked until both explicit refresh operations settle.
      const [licenseResult, catalogResult] = await Promise.allSettled([
        refreshLicense({ force: true }),
        (async () => retryCatalogRefresh())(),
      ]);
      if (licenseResult.status === "fulfilled" && licenseResult.value === null) {
        toast.error(
          "Could not verify the license",
          "The live check did not complete. Check your connection and try again.",
        );
      }
      if (catalogResult.status === "rejected") {
        toast.error("Refresh failed", String(catalogResult.reason));
      }
    } finally {
      setRefreshing(false);
    }
  };

  const handleDeactivate = async () => {
    setDeactivating(true);
    try {
      await deactivateLicense();
      toast.success("License deactivated", "Switched to Free plan.");
      setShowDeactivateConfirm(false);
    } catch (e) {
      if (parseLicenseActivationError(e).code === "cancelled") {
        return;
      }
      // Deactivation errors are unstructured, so only share activation's timeout test.
      if (isPrivilegedCommandTimeoutError(e)) {
        // The native command may still finish after the bridge deadline.
        toast.warning(
          "Deactivation could not be confirmed",
          "SiteCMD stopped waiting for an answer, so it cannot say yet whether this machine was unlinked. If this panel still shows an active license in a moment, choose Refresh License to find out.",
        );
        setShowDeactivateConfirm(false);
        void refreshLicense();
        return;
      }
      const message = errorMessage(e);
      if (message.startsWith(DEACTIVATION_KEYCHAIN_REMNANT)) {
        toast.warning(
          "License unlinked, with one thing left over",
          message.slice(DEACTIVATION_KEYCHAIN_REMNANT.length),
        );
        setShowDeactivateConfirm(false);
        void refreshLicense();
        return;
      }
      toast.error(
        "Deactivation failed",
        userFacingError(e, "Your account is still active. Try again."),
      );
      // Local unlinking may have completed before a later backend failure.
      void refreshLicense();
    } finally {
      setDeactivating(false);
    }
  };

  return (
    <div className="settings-section-stack">
      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Current Plan</h2>
          <span className="text-meta account-plan-badge text-foreground">
            {currentPlanLabel(tier, licenseInfo.billingInterval)}
          </span>
        </div>

        {licenseInfo.isActive ? (
          <>
            <div className="settings-stack">
              <div>
                <p className="section-label-mid account-license-label">Active License</p>
                <div className="settings-inline-card">
                  <Key className="icon-muted account-inline-icon" />
                  <span className="account-license-key text-foreground">••••••••-••••-••••</span>
                </div>
                <p className="subtitle-xs account-license-note">
                  This device is activated. Use the billing portal for invoices, plan changes, and
                  payment details.
                </p>
              </div>

              {licenseInfo.expiresAt && (
                <p className="body-muted">
                  Renews {new Date(licenseInfo.expiresAt).toLocaleDateString()}
                </p>
              )}

              <div>
                {catalogStatus?.active ? (
                  <p className="subtitle-xs account-license-note">
                    Guides updated{" "}
                    {catalogStatus.publishedAt
                      ? new Date(catalogStatus.publishedAt).toLocaleDateString()
                      : catalogStatus.catalogVersion}
                    . Updates download automatically.
                  </p>
                ) : catalogStatus?.error ? (
                  <p className="subtitle-xs account-license-note">
                    The downloaded catalog could not be read, so the built-in baseline guides are in
                    use. The next automatic refresh will retry.
                  </p>
                ) : catalogStatus?.credentialBlock?.code === "cap_reached" ? (
                  <p className="subtitle-xs account-license-note">
                    Your subscription's guide catalog is already active on{" "}
                    {catalogStatus.credentialBlock.active ?? "all"} of{" "}
                    {catalogStatus.credentialBlock.cap ?? "its"} machines, so this one is using the
                    built-in baseline guides. Deactivate the license on a machine you no longer use
                    (from its own Settings), then choose Refresh License here.
                  </p>
                ) : catalogStatus?.credentialBlock?.code === "refused" ? (
                  <p className="subtitle-xs account-license-note">
                    The last catalog check did not get a clear answer from the service, so the
                    built-in baseline guides are in use for now. SiteCMD retries automatically;
                    choosing Refresh License retries right away.
                  </p>
                ) : catalogStatus?.credentialBlock?.code === "subscription_inactive" ? (
                  <p className="subtitle-xs account-license-note">
                    Your subscription is not currently active, so the built-in baseline guides are
                    in use. Choose Manage Billing to sort it out - the full guide catalog comes back
                    on its own once the subscription is active again.
                  </p>
                ) : catalogStatus?.credentialBlock ? (
                  <p className="subtitle-xs account-license-note">
                    The catalog service declined to issue this machine a credential, so the built-in
                    baseline guides are in use. If the license key was mistyped, entering it again
                    fixes it; otherwise contact support and quote{" "}
                    {catalogStatus.credentialBlock.code}.
                  </p>
                ) : catalogStatus && !catalogStatus.endpointConfigured ? (
                  <p className="subtitle-xs account-license-note">
                    This build of SiteCMD was packaged without guide-catalog access, so only the
                    built-in baseline guides are available. Updating to the current release restores
                    the full catalog; if this is the current release, contact {SUPPORT_EMAIL} and
                    quote this message.
                  </p>
                ) : (
                  <p className="subtitle-xs account-license-note">
                    Baseline guides are in use. The full guide catalog downloads automatically in
                    the background.
                  </p>
                )}
              </div>

              <div className="row-wrap">
                <Button onClick={() => void handleRefreshLicense()} disabled={refreshing}>
                  {refreshing ? <Loader2 className="icon-xs animate-spin" /> : "Refresh License"}
                </Button>
                <Button
                  onClick={() => {
                    if (licenseInfo.customerPortalUrl) {
                      openUrl(licenseInfo.customerPortalUrl);
                    }
                  }}
                  disabled={!licenseInfo.customerPortalUrl}
                  variant="outline">
                  Manage Billing
                </Button>
              </div>

              {!showDeactivateConfirm ? (
                <Button
                  onClick={() => setShowDeactivateConfirm(true)}
                  variant="ghost"
                  size="sm"
                  className="account-deactivate-btn text-destructive settings-danger-btn">
                  Deactivate license on this device
                </Button>
              ) : (
                <div className="danger-callout-row">
                  <p className="text-body-muted account-deactivate-warning">
                    This will unlink your license from this device. You can reactivate it anytime.
                  </p>
                  <Button
                    onClick={handleDeactivate}
                    disabled={deactivating}
                    variant="destructive"
                    size="sm"
                    className="account-confirm-btn btn--bold">
                    {deactivating ? <Loader2 className="icon-xs animate-spin" /> : "Confirm"}
                  </Button>
                  <Button
                    onClick={() => setShowDeactivateConfirm(false)}
                    variant="ghost"
                    size="sm"
                    className="account-confirm-btn">
                    Cancel
                  </Button>
                </div>
              )}
            </div>
          </>
        ) : (
          <>
            <div className="settings-stack">
              <p className="body-muted">
                The desktop workbench is free and complete. If you already have a SiteCMD license,
                paste the key from its confirmation email to activate connected-service access on
                this device.
              </p>
              <div className="account-license-input-row">
                <div className="field-shell account-license-field">
                  <Key className="icon-muted account-inline-icon" />
                  <input
                    value={licenseKey}
                    onChange={(e) => setLicenseKey(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleActivate()}
                    placeholder="XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"
                    // Allow formatting variants while bounding pasted input.
                    maxLength={256}
                    className="field-shell__control field-shell__control--mono"
                  />
                </div>
                <Button onClick={handleActivate} disabled={!licenseKey.trim() || activating}>
                  {activating ? <Loader2 className="icon-md animate-spin" /> : "Activate"}
                </Button>
              </div>
            </div>
          </>
        )}
      </section>

      <section className="settings-list">
        <div className="row-between">
          <div>
            <div className="settings-card-title-rule">
              <h2 className="settings-card-title">Connected Service</h2>
            </div>
            <p className="body-muted account-plans-desc">
              {plansExpanded
                ? "The founder beta adds hosted automation around the complete free desktop workbench. Access is comped during the founder beta while usage informs the public pricing pass."
                : "See what the founder beta adds when you want your sites watched between sessions."}
            </p>
          </div>
          {licenseInfo.isActive ? (
            <Button variant="outline" size="sm" onClick={() => setShowPlans((current) => !current)}>
              {plansExpanded ? "Hide Details" : "View Founder Beta"}
            </Button>
          ) : null}
        </div>
        {plansExpanded ? (
          <div className="account-plans-grid">
            <div className="card card--spacious plan-card--highlight">
              <div className="plan-card-head">
                <div className="plan-card-icon plan-card-icon--blue">
                  <Zap className="icon-lg" />
                </div>
                <div className="plan-card-title-wrap">
                  <span className="plan-card-name text-foreground">Founder Beta</span>
                  <p className="subtitle-xs plan-card-desc">
                    Hosted scans, deploy watches, alerts, verification, and CI gates while your
                    laptop is closed.
                  </p>
                </div>
                <span className="text-meta account-plan-badge text-primary">
                  Comped during the founder beta
                </span>
              </div>

              <ul className="plan-card-features">
                {FOUNDER_BETA_FEATURES.map((feature) => (
                  <li key={feature} className="plan-card-feature body-muted">
                    <Check className="icon-sm plan-card-feature-icon text-score-excellent" />
                    {feature}
                  </li>
                ))}
              </ul>

              {licenseInfo.isActive ? (
                <Button disabled className="plan-card-cta text-body">
                  Connected access licensed
                </Button>
              ) : (
                <Button
                  onClick={() => openUrl(FOUNDER_BETA_CONTACT_URL)}
                  className="plan-card-cta text-body">
                  Request founder beta access
                </Button>
              )}
            </div>
          </div>
        ) : null}
      </section>
    </div>
  );
}

export { AccountSection as AccountSettings };
