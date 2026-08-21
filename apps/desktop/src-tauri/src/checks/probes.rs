//! Origin-scoped, per-scan probe transport. Classification and sitemap grammar
//! remain in the portable engine.

use sitecmd_engine::checks::seo::robots::{robots_fetch_from_probe, robots_txt_url};
use sitecmd_engine::checks::seo::sitemap::{
    parse_sitemap_document, sitemap_candidate_urls, sitemap_urls_from_robots, url_is_same_origin,
    SitemapParse, SITEMAP_DECLARATION_PROBE_LIMIT,
};
use sitecmd_engine::probe::ProbeRequest;

use super::{
    performance, probe, CheckContext, RedirectWalk, RobotsTxtFetch, SitemapFetch, SitemapProbe,
    SitemapProbeObservation,
};

/// Per-scan memo for origin-scoped probe fetches shared across async checks.
/// Checks racing on the same entry await one in-flight request instead of each
/// re-fetching the same origin resource.
#[derive(Default)]
pub struct ProbeCache {
    pub(in crate::checks) requested_url: std::sync::OnceLock<url::Url>,
    pub(in crate::checks) robots_txt: tokio::sync::OnceCell<RobotsTxtFetch>,
    pub(in crate::checks) sitemap: tokio::sync::OnceCell<SitemapProbe>,
    pub(in crate::checks) redirect_chain: tokio::sync::OnceCell<RedirectWalk>,
    /// The certificate facts the TLS handshake produced. Memoized like the
    /// other origin probes, and for the same reason: it is one connection per
    /// scan, and what it learned outlives the verdicts it was opened for.
    pub(in crate::checks) tls_facts:
        std::sync::Mutex<Option<sitecmd_engine::checks::security::tls::TlsFacts>>,
}

impl CheckContext {
    /// Follow the scanned URL's redirect chain once per scan and share the
    /// observed hops across checks (`performance.redirect_chain` counts
    /// them, `seo.temporary_redirect` grades their status codes).
    pub async fn redirect_chain(&self) -> &RedirectWalk {
        self.probe_cache
            .redirect_chain
            .get_or_init(|| performance::redirects::walk_redirect_chain(self))
            .await
    }

    /// Fetch `/robots.txt` once per scan through the probe seam and share the
    /// engine-classified outcome across checks.
    pub async fn robots_txt(&self) -> &RobotsTxtFetch {
        self.probe_cache
            .robots_txt
            .get_or_init(|| async {
                let request = ProbeRequest::get(robots_txt_url(&self.url));
                robots_fetch_from_probe(probe(&self.client, request).await)
            })
            .await
    }

    /// Probe the sitemap candidate URLs once per scan. The memo preserves a
    /// conclusive missing result separately from access/network failures.
    pub async fn sitemap(&self) -> &SitemapProbe {
        self.probe_cache
            .sitemap
            .get_or_init(|| async {
                let origin = super::origin_with_port(&self.url);
                // Prefer the sitemap the site declares in robots.txt (the only
                // reliable way to find non-conventional paths, and what stock
                // WordPress + most SEO plugins actually emit), then fall back to
                // the conventional candidate paths. Same-origin only.
                let mut candidates: Vec<String> = Vec::new();
                if let RobotsTxtFetch::Found { body } = self.robots_txt().await {
                    for url in sitemap_urls_from_robots(body)
                        .into_iter()
                        .filter(|url| url_is_same_origin(url, &origin))
                        .take(SITEMAP_DECLARATION_PROBE_LIMIT)
                    {
                        if !candidates.contains(&url) {
                            candidates.push(url);
                        }
                    }
                }
                for url in sitemap_candidate_urls(&origin) {
                    if !candidates.contains(&url) {
                        candidates.push(url);
                    }
                }
                let mut observations = Vec::with_capacity(candidates.len());
                let mut inconclusive = false;
                for url in candidates {
                    let safe_url = crate::log_sanitizer::evidence_safe_page_url(&url);
                    let resp = match self
                        .client
                        .get(&url)
                        .timeout(crate::constants::CHECK_PROBE_TIMEOUT)
                        .send()
                        .await
                    {
                        Ok(response) => response,
                        Err(_) => {
                            inconclusive = true;
                            observations.push(SitemapProbeObservation {
                                url: safe_url,
                                outcome: "request failed".into(),
                            });
                            continue;
                        }
                    };
                    let status = resp.status().as_u16();
                    if matches!(status, 404 | 410) {
                        observations.push(SitemapProbeObservation {
                            url: safe_url,
                            outcome: format!("HTTP {}", status),
                        });
                        continue;
                    }
                    if !resp.status().is_success() {
                        inconclusive = true;
                        observations.push(SitemapProbeObservation {
                            url: safe_url,
                            outcome: format!("HTTP {}", status),
                        });
                        continue;
                    }

                    let body = match crate::http_client::read_text_limited(
                        resp,
                        crate::constants::MAX_SITEMAP_SIZE,
                        crate::constants::CHECK_PROBE_TIMEOUT,
                    )
                    .await
                    {
                        Ok(body) => body,
                        Err(_) => {
                            inconclusive = true;
                            observations.push(SitemapProbeObservation {
                                url: safe_url,
                                outcome: "response body could not be read within probe limits"
                                    .into(),
                            });
                            continue;
                        }
                    };
                    match parse_sitemap_document(&body) {
                        SitemapParse::WellFormed(document) => {
                            return SitemapProbe::Found(SitemapFetch::new(url, body, &document));
                        }
                        // A document the grammar rejects is not a valid
                        // sitemap here even when page discovery can still
                        // read locations out of it.
                        SitemapParse::Salvaged { reason, .. }
                        | SitemapParse::Unusable { reason } => {
                            observations.push(SitemapProbeObservation {
                                url: safe_url,
                                outcome: format!("HTTP {} but {}", status, reason),
                            })
                        }
                    }
                }
                if inconclusive {
                    SitemapProbe::Inconclusive { observations }
                } else {
                    SitemapProbe::Missing { observations }
                }
            })
            .await
    }
}

#[cfg(test)]
#[path = "probes_tests.rs"]
mod tests;
