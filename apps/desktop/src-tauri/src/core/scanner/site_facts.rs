//! Builds baseline observations only from facts gathered by the current scan.
//!
//! Unobserved families remain absent rather than becoming empty observations.

use crate::checks::{CheckContext, CheckResult};
use sitecmd_engine::checks::security::dns_email::{registrable_domain_for_url, DomainTarget};
use sitecmd_engine::dns::DnsOutcome;
use sitecmd_engine::profile::{
    CertificateIdentity, DnsPosture, FieldValue, Observation, OriginSet, SecurityHeaderProfile,
};

/// The check id prefix whose presence proves the DNS questions were asked.
const DNS_CHECK_PREFIX: &str = "security.dns.";

/// Facts collected before the polish phase takes ownership of the page body.
pub(crate) async fn read_before_polish(
    ctx: &CheckContext,
    results: &[CheckResult],
    origin_scoped: bool,
) -> (
    Option<crate::core::page_signals::PageSignals>,
    Option<Observation>,
) {
    let signals = crate::core::page_signals::extract_page_signals_with_headers(
        &ctx.url,
        &ctx.body,
        &ctx.response_headers,
    );
    let observation = observe_site_facts(ctx, results, origin_scoped).await;
    (Some(signals), Some(observation))
}

/// Collect page facts and, when enabled, origin-scoped baseline facts.
pub(crate) async fn observe_site_facts(
    ctx: &CheckContext,
    results: &[CheckResult],
    origin_scoped: bool,
) -> Observation {
    let mut observation = Observation::default();

    observation.push(FieldValue::ThirdPartyOrigins(OriginSet::from_document(
        &ctx.url,
        &ctx.body,
        ctx.body_lower(),
    )));

    if !origin_scoped {
        return observation;
    }

    observation.push(FieldValue::SecurityHeaders(
        SecurityHeaderProfile::from_headers(&ctx.response_headers),
    ));

    if let Some(identity) = ctx
        .observed_tls_facts()
        .as_ref()
        .and_then(CertificateIdentity::from_tls_facts)
    {
        observation.push(FieldValue::Certificate(identity));
    }

    if let Some(posture) = dns_posture(ctx, results).await {
        observation.push(FieldValue::DnsPosture(posture));
    }

    observation
}

/// Build DNS posture only from answers already collected by this scan.
async fn dns_posture(ctx: &CheckContext, results: &[CheckResult]) -> Option<DnsPosture> {
    if !results
        .iter()
        .any(|result| result.check_id.starts_with(DNS_CHECK_PREFIX))
    {
        return None;
    }
    if ctx.is_localhost {
        return None;
    }
    let DomainTarget::Registrable(domain) = registrable_domain_for_url(&ctx.url) else {
        return None;
    };

    use crate::checks::security::dns_email::{
        lookup_caa, lookup_cname_targets, lookup_mx, lookup_txt,
    };
    use sitecmd_engine::checks::security::dns_email::{dangling_cname, dmarc};

    let mx_hosts = match lookup_mx(&domain).await {
        DnsOutcome::Records(records) => records
            .into_iter()
            .map(|record| record.exchange)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let caa_present = matches!(lookup_caa(&domain).await, DnsOutcome::Records(_));
    let cname_target = match lookup_cname_targets(&dangling_cname::www_lookup_name(&domain)).await {
        DnsOutcome::Records(targets) => targets.into_iter().next(),
        _ => None,
    };
    let mut txt = match lookup_txt(&domain).await {
        DnsOutcome::Records(records) => records,
        _ => Vec::new(),
    };
    if let DnsOutcome::Records(records) = lookup_txt(&dmarc::dmarc_lookup_name(&domain)).await {
        txt.extend(records);
    }

    Some(DnsPosture::new(mx_hosts, cname_target, caa_present, txt))
}

#[cfg(test)]
#[path = "site_facts_tests.rs"]
mod tests;
