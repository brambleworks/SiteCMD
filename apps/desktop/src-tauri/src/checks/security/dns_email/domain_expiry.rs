//! RDAP probe transport for portable domain-expiry grading.

use super::domain_target;
use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::dns_email::{
    domain_expiry, skipped_local_result, DomainTarget,
};

pub struct DomainExpiryCheck;

#[async_trait::async_trait]
impl AsyncCheck for DomainExpiryCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.domain_expiry"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn skip_in_predeploy(&self) -> bool {
        true // Registration state is per-domain, not per-build.
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let domain = match domain_target(ctx) {
            DomainTarget::Registrable(domain) => domain,
            DomainTarget::LocalOrIp => {
                return vec![skipped_local_result(
                    domain_expiry::CHECK_ID,
                    domain_expiry::TITLE,
                )]
            }
        };

        let outcome = crate::checks::probe_adapter::probe_with_timeout(
            crate::http_client::client(),
            domain_expiry::rdap_probe(&domain),
            Some(crate::constants::RDAP_LOOKUP_TIMEOUT),
        )
        .await;
        domain_expiry::evaluate_rdap(&domain, &outcome, ctx.evaluation_time)
    }
}
