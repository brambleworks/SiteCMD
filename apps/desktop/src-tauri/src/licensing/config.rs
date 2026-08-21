//! LemonSqueezy tier mappings. Variant ids must match the configured store.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Release builds require injected license IDs; debug builds may omit them.

const STORE_ID_ENV: &str = "SITECMD_LICENSE_STORE_ID";
const CORE_MONTHLY_VARIANT_ID_ENV: &str = "SITECMD_LICENSE_CORE_MONTHLY_VARIANT_ID";
const CORE_ANNUAL_VARIANT_ID_ENV: &str = "SITECMD_LICENSE_CORE_ANNUAL_VARIANT_ID";
const PRO_MONTHLY_VARIANT_ID_ENV: &str = "SITECMD_LICENSE_PRO_MONTHLY_VARIANT_ID";
const PRO_ANNUAL_VARIANT_ID_ENV: &str = "SITECMD_LICENSE_PRO_ANNUAL_VARIANT_ID";
const CORE_CHECKOUT_URL_ENV: &str = "SITECMD_LICENSE_CORE_CHECKOUT_URL";
const PRO_CHECKOUT_URL_ENV: &str = "SITECMD_LICENSE_PRO_CHECKOUT_URL";

const STORE_ID_RAW: Option<&str> = option_env!("SITECMD_LICENSE_STORE_ID");
const CORE_MONTHLY_VARIANT_ID_RAW: Option<&str> =
    option_env!("SITECMD_LICENSE_CORE_MONTHLY_VARIANT_ID");
const CORE_ANNUAL_VARIANT_ID_RAW: Option<&str> =
    option_env!("SITECMD_LICENSE_CORE_ANNUAL_VARIANT_ID");
const PRO_MONTHLY_VARIANT_ID_RAW: Option<&str> =
    option_env!("SITECMD_LICENSE_PRO_MONTHLY_VARIANT_ID");
const PRO_ANNUAL_VARIANT_ID_RAW: Option<&str> =
    option_env!("SITECMD_LICENSE_PRO_ANNUAL_VARIANT_ID");
const CORE_CHECKOUT_URL_RAW: Option<&str> = option_env!("SITECMD_LICENSE_CORE_CHECKOUT_URL");
const PRO_CHECKOUT_URL_RAW: Option<&str> = option_env!("SITECMD_LICENSE_PRO_CHECKOUT_URL");
const CHECKOUT_HOST: &str = "shop.sitecmd.com";

/// Variant IDs for each plan+billing combination.
/// Each maps to a specific LS product variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariantIds {
    pub core_monthly: u64,
    pub core_annual: u64,
    pub pro_monthly: u64,
    pub pro_annual: u64,
}

fn parse_positive_u64_env(name: &str, raw: Option<&str>) -> Result<u64, String> {
    let value = raw.ok_or_else(|| format!("{name} is not set"))?;
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{name} must not be 0"));
    }
    Ok(parsed)
}

fn optional_u64(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default()
}

fn parse_checkout_url_env(name: &str, raw: Option<&str>) -> Result<String, String> {
    let value = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} is not set"))?;
    normalize_checkout_url(value).map_err(|error| format!("{name} {error}"))
}

fn normalize_checkout_url(value: &str) -> Result<String, String> {
    let mut parsed = url::Url::parse(value).map_err(|_| "must be a valid URL".to_string())?;
    if parsed.scheme() != "https" {
        return Err("must use https".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("must not include credentials".to_string());
    }
    if parsed.host_str() != Some(CHECKOUT_HOST) {
        return Err(format!("must be a {CHECKOUT_HOST} checkout URL"));
    }
    if !parsed.path().starts_with("/checkout/buy/") {
        return Err("must use the /checkout/buy/{checkout_id} path".to_string());
    }
    let query_pairs: Vec<_> = parsed
        .query_pairs()
        .filter(|(key, _)| key != "embed")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    parsed.set_query(None);
    if !query_pairs.is_empty() {
        parsed
            .query_pairs_mut()
            .extend_pairs(query_pairs.iter().map(|(key, value)| (&**key, &**value)));
    }
    Ok(parsed.to_string())
}

fn optional_checkout_url(raw: Option<&str>) -> String {
    raw.and_then(|value| normalize_checkout_url(value.trim()).ok())
        .unwrap_or_default()
}

/// LemonSqueezy store ID compiled into this build.
pub fn store_id() -> u64 {
    optional_u64(STORE_ID_RAW)
}

/// LemonSqueezy variant IDs compiled into this build.
pub fn variants() -> VariantIds {
    VariantIds {
        core_monthly: optional_u64(CORE_MONTHLY_VARIANT_ID_RAW),
        core_annual: optional_u64(CORE_ANNUAL_VARIANT_ID_RAW),
        pro_monthly: optional_u64(PRO_MONTHLY_VARIANT_ID_RAW),
        pro_annual: optional_u64(PRO_ANNUAL_VARIANT_ID_RAW),
    }
}

fn validate_variant_id_uniqueness(ids: VariantIds, errors: &mut Vec<String>) {
    let pairs = [
        ("core monthly", ids.core_monthly),
        ("core annual", ids.core_annual),
        ("pro monthly", ids.pro_monthly),
        ("pro annual", ids.pro_annual),
    ];

    for (index, (left_name, left_id)) in pairs.iter().enumerate() {
        for (right_name, right_id) in pairs.iter().skip(index + 1) {
            if left_id == right_id {
                errors.push(format!(
                    "LemonSqueezy variant IDs must be unique ({left_name} and {right_name} both use {left_id})"
                ));
            }
        }
    }
}

/// Validate the paid-license build configuration.
pub fn license_config_errors() -> Vec<String> {
    let mut errors = Vec::new();
    let store = parse_positive_u64_env(STORE_ID_ENV, STORE_ID_RAW);
    let core_monthly =
        parse_positive_u64_env(CORE_MONTHLY_VARIANT_ID_ENV, CORE_MONTHLY_VARIANT_ID_RAW);
    let core_annual =
        parse_positive_u64_env(CORE_ANNUAL_VARIANT_ID_ENV, CORE_ANNUAL_VARIANT_ID_RAW);
    let pro_monthly =
        parse_positive_u64_env(PRO_MONTHLY_VARIANT_ID_ENV, PRO_MONTHLY_VARIANT_ID_RAW);
    let pro_annual = parse_positive_u64_env(PRO_ANNUAL_VARIANT_ID_ENV, PRO_ANNUAL_VARIANT_ID_RAW);
    let core_checkout = parse_checkout_url_env(CORE_CHECKOUT_URL_ENV, CORE_CHECKOUT_URL_RAW);
    let pro_checkout = parse_checkout_url_env(PRO_CHECKOUT_URL_ENV, PRO_CHECKOUT_URL_RAW);

    for result in [
        &store,
        &core_monthly,
        &core_annual,
        &pro_monthly,
        &pro_annual,
    ] {
        if let Err(error) = result {
            errors.push(error.clone());
        }
    }
    for result in [&core_checkout, &pro_checkout] {
        if let Err(error) = result {
            errors.push(error.clone());
        }
    }

    if let (Ok(_), Ok(core_monthly), Ok(core_annual), Ok(pro_monthly), Ok(pro_annual)) =
        (store, core_monthly, core_annual, pro_monthly, pro_annual)
    {
        validate_variant_id_uniqueness(
            VariantIds {
                core_monthly,
                core_annual,
                pro_monthly,
                pro_annual,
            },
            &mut errors,
        );
    }

    errors
}

/// The three subscription tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum Tier {
    Free,
    Core,
    Pro,
}

impl Tier {
    /// Derive tier from a LemonSqueezy variant ID.
    #[tracing::instrument(fields(variant_id))]
    pub fn from_variant_id(variant_id: u64) -> Self {
        let variants = variants();
        if variant_id != 0
            && (variant_id == variants.core_monthly || variant_id == variants.core_annual)
        {
            Tier::Core
        } else if variant_id != 0
            && (variant_id == variants.pro_monthly || variant_id == variants.pro_annual)
        {
            Tier::Pro
        } else {
            // Unknown variant - default to free as a safety measure.
            // This should only happen if variant IDs are misconfigured.
            tracing::warn!(
                "Unknown LemonSqueezy variant_id {}, defaulting to Free tier",
                variant_id
            );
            Tier::Free
        }
    }

    /// Human-readable plan name. Must remain log-free because `Display` can run
    /// while the logger holds a non-reentrant writer lock.
    pub fn plan_name(&self) -> &'static str {
        match self {
            Tier::Free => "Free",
            Tier::Core => "Plus",
            Tier::Pro => "Pro",
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.plan_name())
    }
}

impl std::str::FromStr for Tier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "free" => Ok(Tier::Free),
            "core" | "solo" => Ok(Tier::Core), // "solo" kept for backward compat
            "pro" => Ok(Tier::Pro),
            _ => Err(format!("Unknown tier: {}", s)),
        }
    }
}

// Tiers entitle connected services and catalog access, never local features.

/// Whether the app has real LemonSqueezy store and variant IDs configured.
#[tracing::instrument]
pub fn license_configured() -> bool {
    license_config_errors().is_empty()
}

#[tracing::instrument]
pub fn require_license_configured() -> Result<(), String> {
    if license_configured() {
        Ok(())
    } else {
        Err(format!(
            "LemonSqueezy licensing is not configured for this build. {}",
            license_config_errors().join("; ")
        ))
    }
}

/// Checkout URLs for each paid plan.
#[tracing::instrument]
pub fn core_checkout_url() -> String {
    optional_checkout_url(CORE_CHECKOUT_URL_RAW)
}

#[tracing::instrument]
pub fn pro_checkout_url() -> String {
    optional_checkout_url(PRO_CHECKOUT_URL_RAW)
}

/// LemonSqueezy customer portal URL for managing billing.
#[tracing::instrument]
pub fn customer_portal_url() -> String {
    format!("https://{CHECKOUT_HOST}/billing")
}

/// LemonSqueezy License API base URL.
pub const LICENSE_API_BASE: &str = "https://api.lemonsqueezy.com/v1/licenses";

/// How often to re-validate a license key (24 hours).
pub const VALIDATION_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// Offline grace period before the stale-validation warning fires (7 days).
/// During this window the cached tier is honoured silently; after it expires
/// we enter the final-warning window before the hard downgrade.
pub const OFFLINE_GRACE_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;

/// Final connected-entitlement grace window before downgrade to Free.
pub const FINAL_GRACE_PERIOD_SECS: u64 = 24 * 60 * 60;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_from_variant_id_solo_monthly() {
        // When variant IDs are configured (non-zero), this would map correctly.
        // With zero IDs, unknown variants default to Free.
        let tier = Tier::from_variant_id(999999);
        assert_eq!(tier, Tier::Free);
    }

    #[test]
    fn tier_from_variant_id_zero_never_unlocks_paid_tier() {
        assert_eq!(Tier::from_variant_id(0), Tier::Free);
    }

    #[test]
    fn tier_display_and_parse() {
        assert_eq!(Tier::Free.plan_name(), "Free");
        assert_eq!(Tier::Core.plan_name(), "Plus");
        assert_eq!(Tier::Pro.plan_name(), "Pro");

        assert_eq!("free".parse::<Tier>().unwrap(), Tier::Free);
        assert_eq!("core".parse::<Tier>().unwrap(), Tier::Core);
        assert_eq!("solo".parse::<Tier>().unwrap(), Tier::Core); // backward compat
        assert_eq!("pro".parse::<Tier>().unwrap(), Tier::Pro);
        assert_eq!("Pro".parse::<Tier>().unwrap(), Tier::Pro);
        assert!("invalid".parse::<Tier>().is_err());
    }

    #[test]
    fn no_feature_gating_machinery_exists() {
        let source = include_str!("config.rs");
        for marker in ["enum Feature", "fn has_feature", "FREE_DAILY_SCAN_LIMIT"] {
            assert_eq!(
                source.matches(marker).count(),
                1,
                "{marker} may appear only inside this test's own pin list"
            );
        }
    }

    #[test]
    fn checkout_url_accepts_sitecmd_checkout_buy_links() {
        let url =
            normalize_checkout_url("https://shop.sitecmd.com/checkout/buy/checkout-id").unwrap();
        assert_eq!(url, "https://shop.sitecmd.com/checkout/buy/checkout-id");
    }

    #[test]
    fn checkout_url_strips_embedded_mode_for_browser_windows() {
        let url = normalize_checkout_url(
            "https://shop.sitecmd.com/checkout/buy/checkout-id?discount=LAUNCH&embed=1",
        )
        .unwrap();
        assert_eq!(
            url,
            "https://shop.sitecmd.com/checkout/buy/checkout-id?discount=LAUNCH"
        );
    }

    #[test]
    fn checkout_url_rejects_wrong_origin_or_path() {
        assert!(normalize_checkout_url("https://shop.sitecmd.com/buy/12345").is_err());
        assert!(normalize_checkout_url("https://example.com/checkout/buy/checkout-id").is_err());
        assert!(
            normalize_checkout_url("https://sitecmd.lemonsqueezy.com/checkout/buy/id").is_err()
        );
        assert!(normalize_checkout_url("http://shop.sitecmd.com/checkout/buy/id").is_err());
        assert!(
            normalize_checkout_url("https://user:pass@shop.sitecmd.com/checkout/buy/id").is_err()
        );
    }

    #[test]
    fn customer_portal_uses_sitecmd_shop_domain() {
        assert_eq!(customer_portal_url(), "https://shop.sitecmd.com/billing");
    }

    #[test]
    fn license_config_reports_missing_compile_time_values() {
        if store_id() == 0 {
            let errors = license_config_errors();
            assert!(
                errors.iter().any(|error| error.contains(STORE_ID_ENV)),
                "expected missing store ID error, got {errors:?}"
            );
            assert!(!license_configured());
        }
    }

    #[test]
    fn license_config_rejects_invalid_numbers() {
        assert!(parse_positive_u64_env("TEST_ID", None)
            .unwrap_err()
            .contains("not set"));
        assert!(parse_positive_u64_env("TEST_ID", Some("0"))
            .unwrap_err()
            .contains("must not be 0"));
        assert!(parse_positive_u64_env("TEST_ID", Some("abc"))
            .unwrap_err()
            .contains("positive integer"));
    }

    #[test]
    fn license_config_rejects_duplicate_variant_ids() {
        let mut errors = Vec::new();
        validate_variant_id_uniqueness(
            VariantIds {
                core_monthly: 100,
                core_annual: 100,
                pro_monthly: 200,
                pro_annual: 300,
            },
            &mut errors,
        );

        assert!(
            errors
                .iter()
                .any(|error| error.contains("core monthly") && error.contains("core annual")),
            "expected duplicate variant error, got {errors:?}"
        );
    }

    #[test]
    fn constants_are_sane() {
        assert_eq!(VALIDATION_INTERVAL_SECS, 86400);
        assert_eq!(OFFLINE_GRACE_PERIOD_SECS, 604800);
    }
}
