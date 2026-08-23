//! Broad local-environment inference and strict loopback checks for security decisions.

fn has_host_token(host: &str, token: &str) -> bool {
    host.split('.')
        .flat_map(|label| label.split('-'))
        .any(|part| part == token)
}

fn environment_from_host(host: &str) -> &'static str {
    let lower = host.to_lowercase();

    if lower == "localhost"
        || lower == "127.0.0.1"
        || lower == "0.0.0.0"
        || lower == "::1"
        || lower.ends_with(".local")
        || lower.ends_with(".localhost")
    {
        return "local";
    }

    if has_host_token(&lower, "dev") || has_host_token(&lower, "development") {
        return "development";
    }

    if has_host_token(&lower, "staging")
        || has_host_token(&lower, "stage")
        || has_host_token(&lower, "preview")
        || has_host_token(&lower, "qa")
        || lower.ends_with(".vercel.app")
        || lower.ends_with(".netlify.app")
        || lower.ends_with(".onrender.com")
    {
        return "staging";
    }

    "production"
}

/// Detect if a URL is a localhost/pre-deploy URL (broad - includes *.local, *.localhost)
#[tracing::instrument(skip(url))]
pub fn is_localhost(url: &url::Url) -> bool {
    match url.host_str() {
        Some(host) => environment_from_host(host) == "local",
        None => false,
    }
}

/// True only for exact loopback hosts; `.local` and subdomains may resolve remotely.
#[tracing::instrument(skip(url))]
pub fn is_strict_localhost(url: &url::Url) -> bool {
    match url.host_str() {
        Some(host) => {
            let h = host.to_lowercase();
            h == "localhost" || h == "127.0.0.1" || h == "::1" || h == "[::1]"
        }
        None => false,
    }
}

/// Infer the best default environment label for a URL from its hostname.
#[tracing::instrument(skip(url))]
pub fn infer_environment_name(url: &str) -> &'static str {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(environment_from_host))
        .unwrap_or("production")
}

/// Normalize a user-provided environment label into the canonical set used by SiteCMD.
#[tracing::instrument(fields(value = %value))]
pub fn normalize_environment_name(value: &str) -> Option<&'static str> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" => None,
        "local" | "localhost" | "loopback" => Some("local"),
        "development" | "dev" => Some("development"),
        "staging" | "stage" | "preview" | "qa" | "test" | "testing" => Some("staging"),
        "production" | "prod" | "live" => Some("production"),
        _ => None,
    }
}

/// Resolve a canonical environment, giving localhost URLs precedence over labels.
#[tracing::instrument(skip(url, provided), fields(has_provided = provided.is_some()))]
pub fn resolve_environment_name(url: &str, provided: Option<&str>) -> &'static str {
    let inferred = infer_environment_name(url);
    match provided.and_then(normalize_environment_name) {
        Some(_) if inferred == "local" => "local",
        Some("local") if inferred != "local" => inferred,
        Some("production") if inferred != "production" => inferred,
        Some(normalized) => normalized,
        None => inferred,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localhost_detection() {
        let cases = vec![
            ("http://localhost:3000", true),
            ("http://127.0.0.1:8080", true),
            ("http://0.0.0.0:5173", true),
            ("http://myapp.local", true),
            ("http://myapp.localhost:3000", true),
            ("https://localhost.example.com", false),
            ("https://example.com", false),
            ("https://www.google.com", false),
        ];

        for (url_str, expected) in cases {
            let url = url::Url::parse(url_str).unwrap();
            assert_eq!(is_localhost(&url), expected, "Failed for {}", url_str);
        }
    }

    #[test]
    fn test_strict_localhost_detection() {
        let cases = vec![
            ("http://localhost:3000", true),
            ("http://127.0.0.1:8080", true),
            ("http://[::1]:5173", true),
            // These are localhost but NOT strict-localhost (mDNS/DNS resolved)
            ("http://0.0.0.0:5173", false),
            ("http://myapp.local", false),
            ("http://myapp.localhost:3000", false),
            // Obviously not localhost
            ("https://example.com", false),
            ("https://www.google.com", false),
        ];

        for (url_str, expected) in cases {
            let url = url::Url::parse(url_str).unwrap();
            assert_eq!(
                is_strict_localhost(&url),
                expected,
                "strict failed for {}",
                url_str
            );
        }
    }

    #[test]
    fn test_infer_environment_name() {
        let cases = vec![
            ("http://localhost:3000", "local"),
            ("http://127.0.0.1:8080", "local"),
            ("http://myapp.localhost:3000", "local"),
            ("https://dev.example.com", "development"),
            ("https://marketing-stage.example.com", "staging"),
            ("https://preview-my-app.vercel.app", "staging"),
            ("https://qa.example.com", "staging"),
            ("https://localhost.example.com", "production"),
            ("https://upstage.example.com", "production"),
            ("https://example.com", "production"),
            ("not-a-url", "production"),
        ];

        for (url, expected) in cases {
            assert_eq!(
                infer_environment_name(url),
                expected,
                "infer failed for {}",
                url
            );
        }
    }

    #[test]
    fn test_normalize_environment_name() {
        let cases = vec![
            ("localhost", Some("local")),
            ("dev", Some("development")),
            ("preview", Some("staging")),
            ("qa", Some("staging")),
            ("prod", Some("production")),
            ("live", Some("production")),
            ("", None),
            ("custom", None),
        ];

        for (raw, expected) in cases {
            assert_eq!(
                normalize_environment_name(raw),
                expected,
                "normalize failed for {}",
                raw
            );
        }
    }

    #[test]
    fn test_resolve_environment_name() {
        let cases = vec![
            ("http://127.0.0.1:4321", Some("production"), "local"),
            ("https://dev.example.com", Some("production"), "development"),
            (
                "https://preview-my-app.vercel.app",
                Some("preview"),
                "staging",
            ),
            ("https://localhost.example.com", Some("local"), "production"),
            ("https://upstage.example.com", Some("staging"), "staging"),
            ("https://example.com", Some("prod"), "production"),
            ("https://example.com", Some("staging"), "staging"),
            ("https://example.com", Some("custom"), "production"),
            ("https://qa.example.com", None, "staging"),
        ];

        for (url, provided, expected) in cases {
            assert_eq!(
                resolve_environment_name(url, provided),
                expected,
                "resolve failed for {} with {:?}",
                url,
                provided
            );
        }
    }
}
