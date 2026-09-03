//! Which document a browser payload describes.
//!
//! Every shared payload records the `location.href` it was read from. A
//! runtime that drives a real browser can end up holding a document it never
//! asked for (a subframe it followed as if it were a redirect, a navigation
//! the page started), and grading that document under the target's name is
//! worse than grading nothing. An adapter checks each payload against the
//! documents it admitted before drawing any verdict from it.

use std::collections::BTreeMap;
use url::Url;

/// The documents one navigation was allowed to land on: the analyzed target
/// plus every redirect hop the runtime admitted on the way.
///
/// The host is what a navigation gate decides on, so it is what admission
/// records, and the host is the whole guarantee: a payload from any other
/// host is refused no matter what the runtime observed.
///
/// Within one admitted host the transport is recorded too, but as a weaker
/// promise. A gate that judges the host alone lets a same-host scheme or port
/// change through inline, in either direction, so such a hop is never deferred
/// and never reaches [`Self::admit`]. Each host therefore remembers whether
/// every admission of it was `https`, and [`Self::observe_commit`] folds in
/// what the runtime actually committed. A runtime whose commit signal carries
/// no navigation reason (the desktop webview's does not) cannot tell a
/// server's same-host downgrade from one the page performed on itself, so
/// after such a commit an `http` document on that one host grades. What that
/// buys is the alternative: without it a plain server downgrade fails the
/// browser layer closed on a page that loaded correctly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedDocuments {
    /// Host to `true` when every admission of that host was `https`.
    hosts: BTreeMap<String, bool>,
}

impl AdmittedDocuments {
    pub fn new(target: &Url) -> Self {
        let mut admitted = Self {
            hosts: BTreeMap::new(),
        };
        admitted.admit(target);
        admitted
    }

    /// Record a redirect hop the runtime followed into the main frame.
    pub fn admit(&mut self, hop: &Url) {
        if let Some(host) = document_host(hop) {
            let secure = hop.scheme() == "https";
            self.hosts
                .entry(host)
                .and_modify(|admitted_secure| *admitted_secure &= secure)
                .or_insert(secure);
        }
    }

    /// Record a document the runtime's main frame committed, which refines an
    /// admitted host with the transport actually loaded.
    ///
    /// A commit is something a page can cause, so it only ever narrows: a host
    /// the runtime never admitted stays out, and the most this can do is
    /// relax `https`-only to `http` on a host that was already admitted. It
    /// cannot add a host and it cannot reach another site.
    pub fn observe_commit(&mut self, committed: &Url) {
        let already_admitted =
            document_host(committed).is_some_and(|host: String| self.hosts.contains_key(&host));
        if already_admitted {
            self.admit(committed);
        }
    }

    /// Whether a payload read from `document_url` may be graded as the target.
    pub fn verify(&self, document_url: Option<&str>) -> Result<(), DocumentMismatch> {
        let url = document_url
            .and_then(|raw| Url::parse(raw).ok())
            .ok_or(DocumentMismatch::Unidentified)?;
        let admitted = document_host(&url)
            .and_then(|host| self.hosts.get(&host).copied())
            .is_some_and(|admitted_secure| scheme_is_admitted(admitted_secure, &url));
        if admitted {
            Ok(())
        } else {
            Err(DocumentMismatch::OtherDocument {
                origin: url.origin().ascii_serialization(),
            })
        }
    }

    /// [`Self::verify`] for a payload the caller already decoded. Adapters
    /// that hold the whole payload use this so the identity field is read the
    /// same way everywhere; the desktop analyzer keeps the string form because
    /// it checks the axe payload before parsing it into a report.
    pub fn verify_payload(&self, payload: &serde_json::Value) -> Result<(), DocumentMismatch> {
        self.verify(payload_document_url(payload).as_deref())
    }
}

fn document_host(url: &Url) -> Option<String> {
    url.host_str()
        .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
}

/// A document is only ever `http` or `https`, and `http` only when the
/// runtime admitted this host over `http` too.
fn scheme_is_admitted(admitted_secure: bool, url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => !admitted_secure,
        _ => false,
    }
}

/// Why a payload cannot be graded as the analyzed page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentMismatch {
    /// The payload describes a document on a host the runtime never admitted.
    OtherDocument { origin: String },
    /// The payload carries no usable `document_url`, so it cannot show which
    /// document it describes.
    Unidentified,
}

impl std::fmt::Display for DocumentMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OtherDocument { origin } => {
                write!(formatter, "analyzer graded a different document ({origin})")
            }
            Self::Unidentified => write!(
                formatter,
                "analyzer payload did not identify the document it was read from"
            ),
        }
    }
}

impl std::error::Error for DocumentMismatch {}

/// The `document_url` a shared payload recorded, or `None` when the payload
/// is not an object or predates the field.
pub fn payload_document_url(payload: &serde_json::Value) -> Option<String> {
    payload.get("document_url")?.as_str().map(str::to_string)
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
