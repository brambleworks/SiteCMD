//! System-resolver adapter for dangling www CNAME checks.
//! The engine owns the query sequence and verdict.

use super::{domain_target, lookup_addresses, lookup_cname_targets};
use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::dns_email::{
    dangling_cname, skipped_local_result, DomainTarget,
};

pub struct DanglingCnameCheck;

#[async_trait::async_trait]
impl AsyncCheck for DanglingCnameCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.dns.dangling_cname"
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
                    dangling_cname::CHECK_ID,
                    dangling_cname::TITLE,
                )]
            }
        };

        let cname = lookup_cname_targets(&dangling_cname::www_lookup_name(&domain)).await;
        match dangling_cname::evaluate_www_cname(&domain, cname) {
            dangling_cname::WwwAliasStep::Done(results) => results,
            dangling_cname::WwwAliasStep::LookupTarget(probe) => {
                let addresses = lookup_addresses(probe.target()).await;
                probe.evaluate(addresses)
            }
        }
    }
}
