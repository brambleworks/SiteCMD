//! System-resolver adapters for MX, DNSSEC, and CAA checks.
//! The engine owns apex query interpretation and verdicts.

use super::{domain_target, lookup_caa, lookup_dnskey_count, lookup_mx};
use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::dns_email::{records, skipped_local_result, DomainTarget};

pub struct MxCheck;

#[async_trait::async_trait]
impl AsyncCheck for MxCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.dns.mx"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn skip_in_predeploy(&self) -> bool {
        true // DNS records are per-domain, not per-build; nothing to verify locally.
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let domain = match domain_target(ctx) {
            DomainTarget::Registrable(domain) => domain,
            DomainTarget::LocalOrIp => {
                return vec![skipped_local_result(
                    records::MX_CHECK_ID,
                    records::MX_TITLE,
                )]
            }
        };
        records::evaluate_mx(&domain, lookup_mx(&domain).await)
    }
}

pub struct DnssecCheck;

#[async_trait::async_trait]
impl AsyncCheck for DnssecCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.dns.dnssec"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn skip_in_predeploy(&self) -> bool {
        true // DNS records are per-domain, not per-build; nothing to verify locally.
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let domain = match domain_target(ctx) {
            DomainTarget::Registrable(domain) => domain,
            DomainTarget::LocalOrIp => {
                return vec![skipped_local_result(
                    records::DNSSEC_CHECK_ID,
                    records::DNSSEC_TITLE,
                )]
            }
        };
        records::evaluate_dnssec(&domain, lookup_dnskey_count(&domain).await)
    }
}

pub struct CaaCheck;

#[async_trait::async_trait]
impl AsyncCheck for CaaCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.dns.caa"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn skip_in_predeploy(&self) -> bool {
        true // DNS records are per-domain, not per-build; nothing to verify locally.
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let domain = match domain_target(ctx) {
            DomainTarget::Registrable(domain) => domain,
            DomainTarget::LocalOrIp => {
                return vec![skipped_local_result(
                    records::CAA_CHECK_ID,
                    records::CAA_TITLE,
                )]
            }
        };
        records::evaluate_caa(&domain, lookup_caa(&domain).await)
    }
}
