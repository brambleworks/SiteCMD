//! TLS configuration for requests originating from loopback scans.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};

#[derive(Debug)]
struct LoopbackVerifier(rustls_platform_verifier::Verifier);

impl ServerCertVerifier for LoopbackVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let is_loopback = match server_name {
            ServerName::DnsName(name) => name
                .as_ref()
                .trim_end_matches('.')
                .eq_ignore_ascii_case("localhost"),
            ServerName::IpAddress(address) => std::net::IpAddr::from(*address).is_loopback(),
            _ => false,
        };
        if is_loopback {
            return Ok(ServerCertVerified::assertion());
        }
        self.0
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

pub(super) fn configure(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = rustls_platform_verifier::Verifier::new(provider.clone())
        .expect("Failed to initialize platform certificate verifier");
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("Failed to configure loopback TLS protocols")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(LoopbackVerifier(verifier)))
        .with_no_client_auth();
    // Preconfigured TLS bypasses reqwest's usual protocol negotiation defaults.
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    builder.no_proxy().use_preconfigured_tls(config)
}

#[cfg(test)]
mod tests;
