//! System-resolver adapter for portable DKIM selector verdicts.

use futures_util::future::join_all;

use super::{domain_target, lookup_mx, lookup_txt};
use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::dns_email::{dkim, skipped_local_result, DomainTarget};

pub struct DkimCheck;

#[async_trait::async_trait]
impl AsyncCheck for DkimCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.dns.dkim"
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
                return vec![skipped_local_result(dkim::CHECK_ID, dkim::TITLE)]
            }
        };

        let mx = lookup_mx(&domain).await;
        let apex_txt = lookup_txt(&domain).await;
        match dkim::evaluate_dkim_gate(&domain, &mx, &apex_txt) {
            dkim::DkimStep::Done(results) => results,
            dkim::DkimStep::Sweep(sweep) => {
                let probes = sweep
                    .probe_names()
                    .into_iter()
                    .map(|(selector, name)| async move {
                        (selector.to_string(), lookup_txt(&name).await)
                    });
                let outcomes = join_all(probes).await;
                sweep.evaluate(&outcomes)
            }
        }
    }
}
