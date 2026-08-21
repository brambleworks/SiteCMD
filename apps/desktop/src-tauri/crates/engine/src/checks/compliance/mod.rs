//! Portable compliance checks (sync page analysis).

pub mod consent_mode;
pub mod cookie_consent;
pub mod gdpr;
pub mod legal_documents;
pub mod statements;
pub mod trackers;

/// Multilingual privacy-policy link tokens shared by compliance checks.
/// Keep `PRIVACY_LINK_LANGUAGES` in the confidence copy synchronized.
pub const PRIVACY_LINK_TOKENS: &[&str] = &[
    // English (text, slug, and path forms)
    "privacy policy",
    "privacy-policy",
    "/privacy",
    // German
    "datenschutz",
    // French: "politique de confidentialité" text and unaccented href slugs
    "confidentialité",
    "confidentialite",
    // Spanish: "política de privacidad", "aviso de privacidad"
    "privacidad",
    // Italian footer label
    "informativa sulla privacy",
    "informativa privacy",
    // Portuguese: "política de privacidade"
    "privacidade",
    // Dutch
    "privacybeleid",
    "privacyverklaring",
    // Swedish
    "integritetspolicy",
];

/// True when the (lowercased) page contains a recognizable privacy-policy
/// link signal in any covered language.
pub fn has_privacy_policy_link(lower: &str) -> bool {
    PRIVACY_LINK_TOKENS
        .iter()
        .any(|token| lower.contains(token))
}
