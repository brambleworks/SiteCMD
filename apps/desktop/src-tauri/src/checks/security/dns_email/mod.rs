//! Desktop DNS adapter for the engine's portable email and domain verdicts.
//!
//! One process-wide resolver supplies [`DnsOutcome`] values and reuses its cache
//! across multi-page scans.

pub mod dangling_cname;
pub mod dkim;
pub mod dmarc;
pub mod domain_expiry;
pub mod records;
pub mod spf;

use std::sync::LazyLock;

use hickory_resolver::proto::rr::{Name, RData, RecordType};
use hickory_resolver::TokioResolver;
use sitecmd_engine::checks::security::dns_email::{registrable_domain_for_url, DomainTarget};
use sitecmd_engine::dns::{CaaRecord, DnsOutcome, MxRecord};

/// Where a check should direct its DNS questions, or why it cannot. The
/// desktop treats configured-localhost scans as having no public zone even
/// when the host is a real domain name.
pub(crate) fn domain_target(ctx: &crate::checks::CheckContext) -> DomainTarget {
    if ctx.is_localhost {
        return DomainTarget::LocalOrIp;
    }
    registrable_domain_for_url(&ctx.url)
}

const NO_RESOLVER: &str = "system DNS resolver configuration unavailable";

/// Shared cached resolver using system configuration.
/// `None` disables DNS checks without falling back to public DNS.
static RESOLVER: LazyLock<Option<TokioResolver>> = LazyLock::new(|| {
    let built = TokioResolver::builder_tokio().and_then(|builder| builder.build());
    match built {
        Ok(resolver) => Some(resolver),
        Err(error) => {
            tracing::warn!("dns checks disabled: could not build system resolver: {error}");
            None
        }
    }
});

/// Force a fully-qualified name (trailing dot) so the resolver never appends
/// resolv.conf search domains and answers for the wrong zone - the adapter
/// contract in `sitecmd_engine::dns`.
fn fqdn(name: &str) -> String {
    if name.ends_with('.') {
        name.to_string()
    } else {
        format!("{}.", name)
    }
}

async fn run_lookup<T>(
    kind: &str,
    lookup: impl std::future::Future<Output = Result<T, hickory_resolver::net::NetError>>,
) -> DnsOutcome<T> {
    use hickory_resolver::net::{DnsError, NetError};
    match tokio::time::timeout(crate::constants::DNS_LOOKUP_TIMEOUT, lookup).await {
        Err(_) => DnsOutcome::Failed(format!("{} query timed out", kind)),
        Ok(Err(NetError::Dns(DnsError::NoRecordsFound(_)))) => DnsOutcome::NoRecords,
        Ok(Err(error)) => DnsOutcome::Failed(error.to_string()),
        Ok(Ok(records)) => DnsOutcome::Records(records),
    }
}

/// TXT records at `name`, with each record's character-strings concatenated
/// (long SPF/DKIM values are split into 255-byte segments on the wire).
pub(crate) async fn lookup_txt(name: &str) -> DnsOutcome<Vec<String>> {
    let Some(resolver) = RESOLVER.as_ref() else {
        return DnsOutcome::Failed(NO_RESOLVER.into());
    };
    run_lookup("TXT", resolver.txt_lookup(fqdn(name)))
        .await
        .map_records(|lookup| {
            lookup
                .answers()
                .iter()
                .filter_map(|record| match &record.data {
                    RData::TXT(txt) => Some(
                        txt.txt_data
                            .iter()
                            .map(|segment| String::from_utf8_lossy(segment).into_owned())
                            .collect::<String>(),
                    ),
                    _ => None,
                })
                .collect()
        })
}

/// MX records at `domain`. A null MX (RFC 7505) keeps its root exchange as
/// "." so the verdict can classify the posture.
pub(crate) async fn lookup_mx(domain: &str) -> DnsOutcome<Vec<MxRecord>> {
    let Some(resolver) = RESOLVER.as_ref() else {
        return DnsOutcome::Failed(NO_RESOLVER.into());
    };
    run_lookup("MX", resolver.mx_lookup(fqdn(domain)))
        .await
        .map_records(|lookup| {
            lookup
                .answers()
                .iter()
                .filter_map(|record| match &record.data {
                    RData::MX(mx) => Some(MxRecord {
                        preference: mx.preference,
                        exchange: display_name(&mx.exchange),
                    }),
                    _ => None,
                })
                .collect()
        })
}

fn display_name(name: &Name) -> String {
    if name.is_root() {
        ".".to_string()
    } else {
        name.to_utf8().trim_end_matches('.').to_string()
    }
}

/// Raw records of an arbitrary type at `name`. Filters the answer section
/// to the queried type, so CNAME-chain records never count.
async fn lookup_rdata(name: &str, record_type: RecordType) -> DnsOutcome<Vec<RData>> {
    let Some(resolver) = RESOLVER.as_ref() else {
        return DnsOutcome::Failed(NO_RESOLVER.into());
    };
    run_lookup(
        &record_type.to_string(),
        resolver.lookup(fqdn(name), record_type),
    )
    .await
    .map_records(|lookup| {
        lookup
            .answers()
            .iter()
            .filter(|record| record.record_type() == record_type)
            .map(|record| record.data.clone())
            .collect()
    })
}

/// DNSKEY presence at `name`, as a record count: key material itself never
/// rides into the verdict.
pub(crate) async fn lookup_dnskey_count(name: &str) -> DnsOutcome<usize> {
    match lookup_rdata(name, RecordType::DNSKEY).await {
        DnsOutcome::Records(records) => DnsOutcome::Records(records.len()),
        DnsOutcome::NoRecords => DnsOutcome::NoRecords,
        DnsOutcome::Failed(detail) => DnsOutcome::Failed(detail),
    }
}

/// CAA properties at `name`, reduced to the engine's (tag, value) shape so
/// classification never touches hickory types.
pub(crate) async fn lookup_caa(name: &str) -> DnsOutcome<Vec<CaaRecord>> {
    lookup_rdata(name, RecordType::CAA)
        .await
        .map_records(|records| caa_records(&records))
}

fn caa_records(records: &[RData]) -> Vec<CaaRecord> {
    records
        .iter()
        .filter_map(|rdata| match rdata {
            RData::CAA(caa) => {
                let value = match caa.value_as_issue() {
                    Ok((Some(name), _)) => name.to_utf8().trim_end_matches('.').to_string(),
                    // An empty issuer (`issue ";"`) denies all issuance.
                    Ok((None, _)) => "none (issuance denied)".to_string(),
                    Err(_) => match caa.value_as_iodef() {
                        Ok(url) => url.to_string(),
                        Err(_) => String::from_utf8_lossy(&caa.value).into_owned(),
                    },
                };
                Some(CaaRecord {
                    tag: caa.tag.to_ascii_lowercase(),
                    value,
                })
            }
            _ => None,
        })
        .collect()
}

/// CNAME targets at `name` as display names without trailing dots, in
/// answer order (the first is the alias target).
pub(crate) async fn lookup_cname_targets(name: &str) -> DnsOutcome<Vec<String>> {
    lookup_rdata(name, RecordType::CNAME)
        .await
        .map_records(|records| {
            records
                .iter()
                .filter_map(|rdata| match rdata {
                    RData::CNAME(cname) => Some(display_name(&cname.0)),
                    _ => None,
                })
                .collect()
        })
}

/// A and AAAA addresses for `name` in display form, following CNAME chains.
/// NoRecords means the name authoritatively resolves to no address
/// (NXDOMAIN or no A/AAAA).
pub(crate) async fn lookup_addresses(name: &str) -> DnsOutcome<Vec<String>> {
    let Some(resolver) = RESOLVER.as_ref() else {
        return DnsOutcome::Failed(NO_RESOLVER.into());
    };
    run_lookup("A/AAAA", resolver.lookup_ip(fqdn(name)))
        .await
        .map_records(|lookup| lookup.iter().map(|address| address.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::{domain_target, fqdn};
    use crate::checks::{AsyncCheck, CheckStatus};
    use sitecmd_engine::checks::security::dns_email::DomainTarget;

    fn localhost_ctx() -> crate::checks::CheckContext {
        crate::checks::CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("https://example.com/").expect("static test url"),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: String::new(),
                is_localhost: true,
                is_strict_localhost: false,
                http_version: Some("HTTP/2.0".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(false).clone(),
            probe_cache: Default::default(),
        }
    }

    #[test]
    fn fqdn_appends_exactly_one_trailing_dot() {
        assert_eq!(fqdn("example.com"), "example.com.");
        assert_eq!(fqdn("example.com."), "example.com.");
    }

    #[test]
    fn configured_localhost_scans_have_no_public_zone() {
        assert!(matches!(
            domain_target(&localhost_ctx()),
            DomainTarget::LocalOrIp
        ));
    }

    /// Verifies every localhost shell reports its registered id without network access.
    #[tokio::test]
    async fn every_check_skips_local_hosts_under_its_own_id() {
        let checks: Vec<Box<dyn AsyncCheck>> = vec![
            Box::new(super::spf::SpfCheck),
            Box::new(super::dmarc::DmarcCheck),
            Box::new(super::dkim::DkimCheck),
            Box::new(super::records::MxCheck),
            Box::new(super::records::DnssecCheck),
            Box::new(super::records::CaaCheck),
            Box::new(super::dangling_cname::DanglingCnameCheck),
            Box::new(super::domain_expiry::DomainExpiryCheck),
        ];
        let ctx = localhost_ctx();
        for check in checks {
            let results = check.run(&ctx).await;
            assert_eq!(results.len(), 1, "{}: one skip row", check.id());
            assert_eq!(results[0].check_id, check.id());
            assert_eq!(results[0].status, CheckStatus::Skipped, "{}", check.id());
            assert_eq!(
                results[0].raw_data.as_ref().unwrap()["reason"],
                "local_or_ip_host",
                "{}",
                check.id()
            );
        }
    }
}
