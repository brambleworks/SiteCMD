//! Compliance checks: cookie consent, GDPR, privacy policy, third-party trackers.

pub use sitecmd_engine::checks::compliance::consent_mode;
pub use sitecmd_engine::checks::compliance::cookie_consent;
pub use sitecmd_engine::checks::compliance::gdpr;
pub mod privacy;
pub use sitecmd_engine::checks::compliance::has_privacy_policy_link;
pub use sitecmd_engine::checks::compliance::statements;
pub use sitecmd_engine::checks::compliance::trackers;

use super::{AsyncCheck, Check};

pub fn sync_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(trackers::ThirdPartyTrackerCheck),
        Box::new(trackers::FormConsentCheck),
        Box::new(cookie_consent::CookieConsentCheck),
        Box::new(consent_mode::ConsentModeCheck),
        Box::new(gdpr::DataControllerContactCheck),
        Box::new(gdpr::CookieExpirationCheck),
        Box::new(gdpr::DntRespectCheck),
        Box::new(statements::CcpaNoticeCheck),
        Box::new(statements::AccessibilityStatementCheck),
    ]
}

pub fn async_checks() -> Vec<Box<dyn AsyncCheck>> {
    vec![
        Box::new(privacy::PrivacyPolicyCheck),
        Box::new(privacy::TermsOfServiceCheck),
    ]
}
