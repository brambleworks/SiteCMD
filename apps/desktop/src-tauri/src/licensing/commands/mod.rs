//! License IPC commands and shared lifecycle helpers.

mod license_lifecycle;

use serde::Serialize;
use ts_rs::TS;

use super::access::is_entitled_license_status;
use super::api;
use super::config::{self, Tier};
use super::store::LicenseState;

// Glob re-exports are required so `tauri::generate_handler!` can resolve the
// `__cmd__*` / `__tauri_command_name_*` helpers that `#[tauri::command]` emits
// into the submodule namespace.
pub use license_lifecycle::*;

/// Frontend warning state for cached-license validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum ValidationWarning {
    /// Validation succeeded within the last VALIDATION_INTERVAL_SECS window.
    None,
    /// Validation is failing within the offline grace period.
    Stale,
    /// Validation is failing within the final grace period.
    StaleFinalWarning,
    /// The local secret store could not provide the license key.
    KeyUnreadable,
    /// The key is entitled, but this machine's instance is deactivated.
    InstanceDeactivated,
}

impl ValidationWarning {
    pub fn as_str(self) -> &'static str {
        match self {
            ValidationWarning::None => "none",
            ValidationWarning::Stale => "stale",
            ValidationWarning::StaleFinalWarning => "stale_final_warning",
            ValidationWarning::KeyUnreadable => "key_unreadable",
            ValidationWarning::InstanceDeactivated => "instance_deactivated",
        }
    }
}

/// Billing cadence for paid LemonSqueezy subscription variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum BillingInterval {
    Monthly,
    Yearly,
}

/// License info returned to the frontend.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct LicenseInfo {
    /// Current tier: "free", "core", or "pro".
    pub tier: Tier,
    /// License status: "active", "expired", "disabled", or "none".
    pub status: String,
    /// Human-readable plan name.
    pub plan_name: String,
    /// Paid subscription billing cadence, when the variant ID is known.
    pub billing_interval: Option<BillingInterval>,
    /// Whether the user has an active paid subscription.
    pub is_active: bool,
    /// Subscription expiry date if known.
    pub expires_at: Option<String>,
    /// Checkout URLs for upgrade.
    pub checkout_urls: CheckoutUrls,
    /// URL for managing billing (LemonSqueezy customer portal).
    pub customer_portal_url: String,
    /// Whether the most recent validation attempt(s) succeeded. Drives the
    /// validation-stale banner in the desktop UI. Defaults to `None` for
    /// fresh validation and for the Free tier (no license to validate).
    pub validation_warning: ValidationWarning,
}

/// Checkout URLs for the frontend to open in the system browser.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct CheckoutUrls {
    pub core: String,
    pub pro: String,
    pub core_monthly: String,
    pub core_annual: String,
    pub pro_monthly: String,
    pub pro_annual: String,
}

pub(super) fn checkout_urls() -> CheckoutUrls {
    let core = config::core_checkout_url();
    let pro = config::pro_checkout_url();
    CheckoutUrls {
        core: core.clone(),
        pro: pro.clone(),
        core_monthly: core.clone(),
        core_annual: core,
        pro_monthly: pro.clone(),
        pro_annual: pro,
    }
}

fn billing_interval_from_variant_id_with_variants(
    variant_id: u64,
    variants: config::VariantIds,
) -> Option<BillingInterval> {
    if variant_id == 0 {
        return None;
    }
    if variant_id == variants.core_monthly || variant_id == variants.pro_monthly {
        Some(BillingInterval::Monthly)
    } else if variant_id == variants.core_annual || variant_id == variants.pro_annual {
        Some(BillingInterval::Yearly)
    } else {
        None
    }
}

pub(super) fn billing_interval_from_variant_id(variant_id: u64) -> Option<BillingInterval> {
    billing_interval_from_variant_id_with_variants(variant_id, config::variants())
}

pub(super) fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub(super) fn parse_iso(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

/// Free tier info - returned when no license is active.
pub(super) fn free_info() -> LicenseInfo {
    LicenseInfo {
        tier: Tier::Free,
        status: "none".to_string(),
        plan_name: "Free".to_string(),
        billing_interval: None,
        is_active: false,
        expires_at: None,
        checkout_urls: checkout_urls(),
        customer_portal_url: config::customer_portal_url(),
        validation_warning: ValidationWarning::None,
    }
}

/// Build LicenseInfo from a persisted state. `warning` reflects how stale the
/// last successful validation is; pass `ValidationWarning::None` when the
/// caller has no reason to surface a banner.
pub(super) fn info_from_state(state: &LicenseState) -> LicenseInfo {
    info_from_state_with_warning(state, ValidationWarning::None)
}

pub(super) fn info_from_state_with_warning(
    state: &LicenseState,
    warning: ValidationWarning,
) -> LicenseInfo {
    // Instance deactivation takes precedence over staleness because reconnecting
    // the license key, not retrying validation, is the available recovery.
    let warning = if state.status == INSTANCE_DEACTIVATED_STATUS {
        ValidationWarning::InstanceDeactivated
    } else {
        warning
    };
    let is_active = is_entitled_license_status(&state.status);
    let tier = if is_active { state.tier } else { Tier::Free };
    LicenseInfo {
        tier,
        status: state.status.clone(),
        plan_name: if is_active {
            state.tier.plan_name().to_string()
        } else {
            "Free".to_string()
        },
        billing_interval: if is_active {
            billing_interval_from_variant_id(state.variant_id)
        } else {
            None
        },
        is_active,
        expires_at: state.expires_at.clone(),
        checkout_urls: checkout_urls(),
        customer_portal_url: config::customer_portal_url(),
        validation_warning: warning,
    }
}

/// Classify cached validation age; only `Expired` permits downgrade to Free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineValidationState {
    /// Validation was successful within the last interval; nothing to surface.
    Fresh,
    /// Validation has been failing but the cached tier is honoured silently
    /// or with a soft "couldn't reach license server" banner (within
    /// OFFLINE_GRACE_PERIOD_SECS).
    Stale,
    /// Cached tier is still honoured but the user is in the final-warning
    /// window (within OFFLINE_GRACE_PERIOD_SECS + FINAL_GRACE_PERIOD_SECS).
    StaleFinalWarning,
    /// Both grace windows have elapsed; safe to downgrade to Free.
    Expired,
}

pub(super) fn classify_offline_validation(
    last_validated_at: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> OfflineValidationState {
    use config::{FINAL_GRACE_PERIOD_SECS, OFFLINE_GRACE_PERIOD_SECS, VALIDATION_INTERVAL_SECS};

    let Some(last) = parse_iso(last_validated_at) else {
        return OfflineValidationState::Expired;
    };
    let elapsed = now.signed_duration_since(last).num_seconds();
    if elapsed < 0 {
        // Clock skew or tampering: refuse to honour a cached tier whose
        // timestamp lies ahead of "now". Treat as fully expired.
        return OfflineValidationState::Expired;
    }
    let elapsed = elapsed as u64;
    if elapsed <= VALIDATION_INTERVAL_SECS {
        OfflineValidationState::Fresh
    } else if elapsed <= OFFLINE_GRACE_PERIOD_SECS {
        OfflineValidationState::Stale
    } else if elapsed <= OFFLINE_GRACE_PERIOD_SECS + FINAL_GRACE_PERIOD_SECS {
        OfflineValidationState::StaleFinalWarning
    } else {
        OfflineValidationState::Expired
    }
}

/// Local marker for an entitled key whose machine instance was deactivated.
pub(super) const INSTANCE_DEACTIVATED_STATUS: &str = "deactivated";

pub(super) fn state_refreshed_from_validation_result(
    mut state: LicenseState,
    result: api::LicenseResult,
    validated_at: String,
) -> LicenseState {
    state.status = if result.valid {
        if is_entitled_license_status(&result.status) {
            result.status
        } else {
            // Trust the provider's aggregate valid verdict, but warn if its key
            // status contradicts entitlement.
            tracing::warn!(
                "validate answered valid:true with non-entitled status {:?}; trusting the verdict",
                result.status
            );
            "active".to_string()
        }
    } else if is_entitled_license_status(&result.status) {
        // `valid:false` overrides the license object's aggregate status because
        // this machine may have been deactivated while the license remains active.
        INSTANCE_DEACTIVATED_STATUS.to_string()
    } else {
        result.status
    };
    state.last_validated_at = validated_at;

    // Preserve adopted server tiers; activation already rejects unknown variants.

    if let Some(expires_at) = result.expires_at {
        state.expires_at = Some(expires_at);
    }

    state
}

#[cfg(debug_assertions)]
pub(super) fn dev_info_from_state_when_licensing_unconfigured(
    state: &LicenseState,
) -> Option<LicenseInfo> {
    if !state.license_key.starts_with("sitecmd-dev-") {
        return None;
    }

    // Dev license sentinels bypass remote validation and offline grace.
    if !is_entitled_license_status(&state.status) {
        return None;
    }

    Some(info_from_state(state))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
