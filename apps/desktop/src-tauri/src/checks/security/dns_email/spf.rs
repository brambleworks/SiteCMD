//! System-resolver adapter for the engine's SPF verdict.

use super::{domain_target, lookup_mx, lookup_txt};
use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::dns_email::{skipped_local_result, spf, DomainTarget};

pub struct SpfCheck;

#[async_trait::async_trait]
impl AsyncCheck for SpfCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.dns.spf"
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
                return vec![skipped_local_result(spf::CHECK_ID, spf::TITLE)]
            }
        };

        match spf::evaluate_spf_txt(&domain, lookup_txt(&domain).await) {
            spf::SpfStep::Done(results) => results,
            spf::SpfStep::NeedsMx(pending) => {
                let mx = lookup_mx(pending.domain()).await;
                pending.evaluate(&mx)
            }
        }
    }
}
