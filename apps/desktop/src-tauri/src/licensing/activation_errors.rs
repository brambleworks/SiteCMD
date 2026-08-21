//! Typed activation errors and license-key normalization.
//!
//! Known provider shapes map to a closed frontend contract. Unrecognized but
//! conclusive provider refusals retain bounded provider text.

use serde::Serialize;

/// Maximum normalized key length accepted across IPC.
pub const MAX_LICENSE_KEY_LENGTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseActivationErrorCode {
    /// User submitted an empty / whitespace-only key.
    KeyRequired,
    /// Key remains malformed after normalization.
    InvalidKey,
    /// LemonSqueezy says the key doesn't exist.
    NotFound,
    /// Key exists but is associated with a different LS store than this build.
    StoreMismatch,
    /// Activation slot limit reached on the LemonSqueezy side.
    LimitReached,
    /// The subscription behind the key has ended.
    Expired,
    /// Key recognised, but the variant ID doesn't map to a known tier.
    VariantUnknown,
    /// Conclusive provider verdict with no more specific local code.
    ProviderRefused,
    /// Server returned 4xx / 5xx without a recognisable message.
    ServerError,
    /// Couldn't reach the LemonSqueezy API at all (network failure).
    Network,
    /// Activation succeeded with no `instance_id` returned.
    MissingInstanceId,
    /// Another activation replaced the captured generation.
    ChangedDuringActivation,
    /// The user declined replacement confirmation.
    Cancelled,
    /// The attempt definitively stopped locally before completion.
    Incomplete,
    /// Catch-all when none of the above match.
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct LicenseActivationErrorPayload {
    pub code: LicenseActivationErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl LicenseActivationErrorPayload {
    pub fn new(code: LicenseActivationErrorCode) -> Self {
        Self {
            code,
            message: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"code":"unknown","message":"failed to serialize activation error"}"#.to_string()
        })
    }
}

/// Strip whitespace and zero-width paste artifacts without changing case.
pub fn normalize_license_key(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && !is_zero_width(*c))
        .take(MAX_LICENSE_KEY_LENGTH)
        .collect()
}

fn is_zero_width(c: char) -> bool {
    matches!(
        c,
        '\u{200B}' // zero-width space
        | '\u{200C}' // zero-width non-joiner
        | '\u{200D}' // zero-width joiner
        | '\u{2060}' // word joiner
        | '\u{FEFF}' // zero-width no-break space / BOM
    )
}

/// Classify raw activation errors into the frontend contract.
pub fn classify_activation_error(raw: &str) -> LicenseActivationErrorCode {
    classify_known(raw).unwrap_or(LicenseActivationErrorCode::Unknown)
}

/// Classify authoritative provider text, preserving conclusive unknown refusals.
pub fn classify_provider_refusal(raw: &str) -> LicenseActivationErrorCode {
    classify_known(raw).unwrap_or(LicenseActivationErrorCode::ProviderRefused)
}

/// Shared vocabulary; callers choose different fallbacks for `None`.
fn classify_known(raw: &str) -> Option<LicenseActivationErrorCode> {
    let lowered = raw.to_ascii_lowercase();
    // Parse failures may echo attacker-controlled vocabulary, so classify them first.
    if lowered.contains("failed to parse activation response")
        || lowered.contains("carried no provider verdict")
    {
        return Some(LicenseActivationErrorCode::ServerError);
    }
    if lowered.contains("license activation request failed")
        || lowered.contains("failed to read activation response")
        || lowered.contains("activation request failed")
        || lowered.contains("timed out")
        || lowered.contains("dns")
        || lowered.contains("connection")
    {
        return Some(LicenseActivationErrorCode::Network);
    }
    if lowered.contains("not found") || lowered.contains("invalid license key") {
        return Some(LicenseActivationErrorCode::NotFound);
    }
    if lowered.contains("activation limit")
        || lowered.contains("already activated")
        || lowered.contains("instances limit")
        || lowered.contains("limit reached")
    {
        return Some(LicenseActivationErrorCode::LimitReached);
    }
    if lowered.contains("expired") {
        return Some(LicenseActivationErrorCode::Expired);
    }
    if lowered.contains("belongs to store") || lowered.contains("store id") {
        return Some(LicenseActivationErrorCode::StoreMismatch);
    }
    if lowered.contains("variant is not recognized") || lowered.contains("variant not recognized") {
        return Some(LicenseActivationErrorCode::VariantUnknown);
    }
    if lowered.contains("no instance_id") {
        return Some(LicenseActivationErrorCode::MissingInstanceId);
    }
    if lowered.contains("activation failed (http") {
        return Some(LicenseActivationErrorCode::ServerError);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_whitespace_and_zero_width() {
        assert_eq!(
            normalize_license_key("  ABCD-1234\u{200B}-EFGH \n"),
            "ABCD-1234-EFGH"
        );
    }

    #[test]
    fn normalize_strips_bom() {
        assert_eq!(normalize_license_key("\u{FEFF}ABCD-1234"), "ABCD-1234");
    }

    #[test]
    fn normalize_preserves_case() {
        // LS treats keys as opaque tokens; do not canonicalise case.
        assert_eq!(normalize_license_key("aBcD-1234"), "aBcD-1234");
    }

    #[test]
    fn normalize_caps_length() {
        let huge = "A".repeat(500);
        assert_eq!(normalize_license_key(&huge).len(), MAX_LICENSE_KEY_LENGTH);
    }

    #[test]
    fn classify_maps_network_failures() {
        assert_eq!(
            classify_activation_error("License activation request failed: dns lookup failed"),
            LicenseActivationErrorCode::Network,
        );
        assert_eq!(
            classify_activation_error("Failed to read activation response: connection reset"),
            LicenseActivationErrorCode::Network,
        );
    }

    #[test]
    fn classify_maps_not_found() {
        assert_eq!(
            classify_activation_error("license_key not found"),
            LicenseActivationErrorCode::NotFound,
        );
        assert_eq!(
            classify_activation_error("Invalid license key"),
            LicenseActivationErrorCode::NotFound,
        );
    }

    #[test]
    fn classify_maps_expired() {
        assert_eq!(
            classify_activation_error("This license key is expired."),
            LicenseActivationErrorCode::Expired,
        );
        assert_eq!(
            classify_activation_error("License key expired"),
            LicenseActivationErrorCode::Expired,
        );
    }

    #[test]
    fn classify_maps_limit_reached() {
        assert_eq!(
            classify_activation_error("activation limit reached for this key"),
            LicenseActivationErrorCode::LimitReached,
        );
        assert_eq!(
            classify_activation_error("Instances limit exceeded"),
            LicenseActivationErrorCode::LimitReached,
        );
    }

    #[test]
    fn classify_maps_store_mismatch() {
        assert_eq!(
            classify_activation_error("License key belongs to store 99 but expected 1"),
            LicenseActivationErrorCode::StoreMismatch,
        );
    }

    #[test]
    fn a_serde_echo_of_provider_vocabulary_is_not_a_verdict() {
        // Prove echoed body text cannot masquerade as a provider verdict.
        for (body, vocabulary) in [
            (r#"{"valid":"not found"}"#, "not found"),
            (r#"{"valid":"limit reached"}"#, "limit reached"),
            (r#"{"activated":"expired"}"#, "expired"),
            (r#"{"deactivated":"belongs to store"}"#, "belongs to store"),
        ] {
            let error = crate::licensing::api::parse_activate_response(body, 1, 400)
                .expect_err("a type-mismatched body must fail the parse");
            assert!(
                error.to_ascii_lowercase().contains(vocabulary),
                "fixture stopped echoing its vocabulary: {error}"
            );
            assert_eq!(
                classify_activation_error(&error),
                LicenseActivationErrorCode::ServerError,
                "an echoed body string must not classify as a provider verdict: {error}"
            );
        }
    }

    #[test]
    fn an_unrecognized_provider_verdict_stays_conclusive() {
        for reason in [
            "This license key has been disabled.",
            "This license key was refunded.",
            "This order is on hold pending review.",
            "Activation failed",
        ] {
            assert_eq!(
                classify_activation_error(reason),
                LicenseActivationErrorCode::Unknown,
                "fixture stopped being unrecognized, so it no longer tests the fallback: {reason}"
            );
            assert_eq!(
                classify_provider_refusal(reason),
                LicenseActivationErrorCode::ProviderRefused,
                "a provider verdict must not be reconcilable: {reason}"
            );
        }
    }

    #[test]
    fn the_provider_fallback_never_overrides_a_recognized_code() {
        for reason in [
            "This license key has reached its activation limit.",
            "This license key is expired.",
            "License key not found",
            "License key belongs to store 99 but expected 1",
            "Activation failed (http 422)",
        ] {
            assert_eq!(
                classify_provider_refusal(reason),
                classify_activation_error(reason),
                "the provider fallback changed a recognized classification: {reason}"
            );
            assert_ne!(
                classify_provider_refusal(reason),
                LicenseActivationErrorCode::ProviderRefused,
                "a recognized reason must keep its own code: {reason}"
            );
        }
    }

    #[test]
    fn classify_falls_back_to_unknown_for_unrecognized_messages() {
        assert_eq!(
            classify_activation_error("totally novel error from a future LS version"),
            LicenseActivationErrorCode::Unknown,
        );
    }

    #[test]
    fn payload_serializes_code_only_when_no_message() {
        let json = LicenseActivationErrorPayload::new(LicenseActivationErrorCode::Network)
            .to_json_string();
        assert_eq!(json, r#"{"code":"network"}"#);
    }

    #[test]
    fn incomplete_serializes_to_the_string_the_frontend_matches() {
        let json = LicenseActivationErrorPayload::new(LicenseActivationErrorCode::Incomplete)
            .to_json_string();
        assert_eq!(json, r#"{"code":"incomplete"}"#);
    }

    #[test]
    fn payload_includes_message_when_set() {
        let json = LicenseActivationErrorPayload::new(LicenseActivationErrorCode::LimitReached)
            .with_message("4/4 instances")
            .to_json_string();
        assert!(json.contains(r#""code":"limit_reached""#));
        assert!(json.contains(r#""message":"4/4 instances""#));
    }
}
