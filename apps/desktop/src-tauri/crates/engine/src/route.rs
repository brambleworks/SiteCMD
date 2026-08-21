//! Versioned route canonicalization shared across runtimes.
//!
//! Version 1 preserves case and slash distinctions, resolves dot segments,
//! normalizes safe percent encoding, and strips query strings and fragments.

use serde::{Deserialize, Serialize};

/// Payload canonicalizer version. Rule changes require a new compatibility
/// version so stored route identities retain their original meaning.
pub const CANONICALIZER_VERSION: u8 = 1;

/// ASCII characters RFC 3986 calls unreserved. Percent-encoding any of these
/// is legal but meaningless, so the canonical form decodes them.
fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

/// One route: a canonical path plus whether the observation that produced it
/// carried a query string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CanonicalRoute {
    pub route: String,
    #[serde(default)]
    pub query_dependent: bool,
}

impl CanonicalRoute {
    pub fn new(route: impl Into<String>, query_dependent: bool) -> Self {
        Self {
            route: route.into(),
            query_dependent,
        }
    }
}

/// Canonicalize an observed URL into a route.
///
/// Takes the FINAL url after redirects: identity belongs to the resource the
/// scanner actually read, not the one it asked for.
pub fn canonical_route(url: &url::Url) -> CanonicalRoute {
    CanonicalRoute {
        route: canonical_path(url.path()),
        query_dependent: url.query().is_some_and(|query| !query.is_empty()),
    }
}

/// Canonicalize an authored or stored route path without observation metadata.
pub fn canonical_path(path: &str) -> String {
    let without_fragment = path.split('#').next().unwrap_or("");
    let without_query = without_fragment.split('?').next().unwrap_or("");
    let normalized = normalize_percent_encoding(without_query);
    let resolved = resolve_dot_segments(&normalized);
    if resolved.starts_with('/') {
        resolved
    } else {
        format!("/{resolved}")
    }
}

/// Uppercase the hex digits of every escape and decode the ones that encode
/// an unreserved character, leaving `%2F` and any non-ASCII byte alone.
fn normalize_percent_encoding(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = String::with_capacity(path.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte != b'%' || index + 2 >= bytes.len() {
            out.push(byte as char);
            index += 1;
            continue;
        }
        let (high, low) = (bytes[index + 1], bytes[index + 2]);
        let Some(decoded) = hex_pair(high, low) else {
            // Not a valid escape. Left exactly as sent: rewriting a malformed
            // path would invent a route the server never served.
            out.push('%');
            index += 1;
            continue;
        };
        if is_unreserved(decoded) {
            out.push(decoded as char);
        } else {
            out.push('%');
            out.push(high.to_ascii_uppercase() as char);
            out.push(low.to_ascii_uppercase() as char);
        }
        index += 3;
    }
    out
}

fn hex_pair(high: u8, low: u8) -> Option<u8> {
    let value = |digit: u8| match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    };
    Some(value(high)? * 16 + value(low)?)
}

/// RFC 3986 dot-segment removal. Repeated slashes are NOT collapsed: `//a`
/// and `/a` can serve different content, and the rules say what is assumed
/// rather than what looks tidy.
fn resolve_dot_segments(path: &str) -> String {
    let leading_slash = path.starts_with('/');
    let trailing_slash = path.ends_with('/') && path.len() > 1;
    let mut segments: Vec<&str> = Vec::new();
    let mut ends_in_dot_segment = false;
    for (index, segment) in path.split('/').enumerate() {
        match segment {
            "." => ends_in_dot_segment = true,
            ".." => {
                segments.pop();
                ends_in_dot_segment = true;
            }
            other => {
                // Only the empty segment BEFORE the leading slash is
                // dropped. Every later one is a repeated slash, which is
                // preserved: `//a` and `/a` can serve different content.
                if !(index == 0 && leading_slash) {
                    segments.push(other);
                }
                ends_in_dot_segment = false;
            }
        }
    }
    let mut out = String::new();
    if leading_slash {
        out.push('/');
    }
    out.push_str(&segments.join("/"));
    if (trailing_slash || ends_in_dot_segment) && !out.ends_with('/') {
        out.push('/');
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
