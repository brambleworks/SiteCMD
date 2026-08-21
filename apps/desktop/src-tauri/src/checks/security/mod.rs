//! Web security checks. Legacy `security.vibe.*` IDs remain for stored-result
//! compatibility and report observable page evidence only.

pub use sitecmd_engine::checks::security::cookies;
pub mod cors;
pub use sitecmd_engine::checks::security::cross_origin;
pub mod directory_listing;
pub mod dns_email;
pub use sitecmd_engine::checks::security::email_exposure;
pub mod exposed_files;
pub use sitecmd_engine::checks::security::forms;
pub use sitecmd_engine::checks::security::headers;
pub mod https_enforcement;
pub use sitecmd_engine::checks::security::mixed_content;
pub mod open_redirect;
pub mod security_txt;
pub use sitecmd_engine::checks::security::server_info;
pub use sitecmd_engine::checks::security::sri;
pub mod ssl;
pub mod vulnerable_libraries;

// Legacy `security.vibe.*` checks; identifiers are retained for stored-result compatibility.
pub use sitecmd_engine::checks::security::client_auth;
pub use sitecmd_engine::checks::security::csrf;
pub use sitecmd_engine::checks::security::env_exposure;
pub use sitecmd_engine::checks::security::exposed_keys;
pub use sitecmd_engine::checks::security::hardcoded_secrets;

use super::{AsyncCheck, Check};

/// Returns all synchronous security checks (analyze already-fetched data)
pub fn sync_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(headers::SecurityHeadersCheck),
        Box::new(cross_origin::CrossOriginIsolationCheck),
        Box::new(mixed_content::MixedContentCheck),
        Box::new(cookies::CookieSecurityCheck),
        Box::new(email_exposure::EmailExposureCheck),
        Box::new(server_info::ServerInfoCheck),
        Box::new(sri::SubresourceIntegrityCheck),
        Box::new(cors::CorsCheck),
        Box::new(forms::InsecureFormCheck),
        Box::new(forms::FormActionHijackCheck),
        // Vibe-coder checks
        Box::new(exposed_keys::ExposedApiKeysCheck),
        Box::new(hardcoded_secrets::HardcodedSecretsCheck),
        Box::new(client_auth::ClientAuthCheck),
        Box::new(csrf::CsrfCheck),
        Box::new(env_exposure::EnvExposureCheck),
    ]
}

/// Returns all async security checks (make additional HTTP requests)
pub fn async_checks() -> Vec<Box<dyn AsyncCheck>> {
    vec![
        Box::new(ssl::SslCheck),
        Box::new(security_txt::SecurityTxtCheck),
        Box::new(https_enforcement::HttpsEnforcementCheck),
        Box::new(cors::CorsReflectionProbeCheck),
        Box::new(exposed_files::ExposedFilesCheck),
        Box::new(directory_listing::DirectoryListingCheck),
        Box::new(open_redirect::OpenRedirectCheck),
        Box::new(vulnerable_libraries::VulnerableLibrariesCheck),
        // DNS / email / domain checks (share one cached resolver)
        Box::new(dns_email::spf::SpfCheck),
        Box::new(dns_email::dmarc::DmarcCheck),
        Box::new(dns_email::dkim::DkimCheck),
        Box::new(dns_email::records::MxCheck),
        Box::new(dns_email::records::DnssecCheck),
        Box::new(dns_email::records::CaaCheck),
        Box::new(dns_email::dangling_cname::DanglingCnameCheck),
        Box::new(dns_email::domain_expiry::DomainExpiryCheck),
    ]
}
