export function desktopOAuthSafetyFailures(read) {
  const buildScript = read("apps/desktop/src-tauri/build.rs");
  const oauthCommands = read("apps/desktop/src-tauri/src/commands/oauth.rs");
  const googleOAuth = read("apps/desktop/src-tauri/src/integrations/google_oauth.rs");
  const githubOAuth = read("apps/desktop/src-tauri/src/integrations/github_oauth.rs");
  const googleOAuthProduction = googleOAuth.split("#[cfg(test)]")[0];
  const githubOAuthProduction = githubOAuth.split("#[cfg(test)]")[0];
  const productionSources = [buildScript, oauthCommands, googleOAuthProduction];
  const failures = [];

  if (
    productionSources.some(
      (source) => source.includes("GOOGLE_CLIENT_SECRET") || source.includes("client_secret"),
    )
  ) {
    failures.push(
      "Desktop Google OAuth must use a native public client with PKCE and must never bake in or transmit GOOGLE_CLIENT_SECRET.",
    );
  }

  if (
    !googleOAuthProduction.includes("generate_pkce_pair") ||
    !googleOAuthProduction.includes('("code_verifier", code_verifier)') ||
    !oauthCommands.includes("&pkce.challenge") ||
    !oauthCommands.includes("&pending.code_verifier")
  ) {
    failures.push(
      "Desktop Google OAuth must retain PKCE generation, authorization challenge, and token-exchange verifier binding.",
    );
  }

  if (
    !githubOAuthProduction.includes('pub const SCOPES: &[&str] = &["repo"];') ||
    githubOAuthProduction.includes('"read:org"')
  ) {
    failures.push(
      "Desktop GitHub classic OAuth must not request unrelated organization scope; keep the current integration limited to repo until it migrates to a fine-grained GitHub App.",
    );
  }

  if (
    [googleOAuthProduction, githubOAuthProduction].some(
      (source) =>
        source.includes("crate::http_client::client()") ||
        source.includes(".json().await") ||
        source.includes(".text().await"),
    ) ||
    !googleOAuthProduction.includes("credentialed_service_client") ||
    !githubOAuthProduction.includes("credentialed_service_client") ||
    !googleOAuthProduction.includes("OAUTH_RESPONSE_MAX_BYTES") ||
    !githubOAuthProduction.includes("OAUTH_RESPONSE_MAX_BYTES") ||
    !googleOAuthProduction.includes("read_json_limited") ||
    !githubOAuthProduction.includes("read_json_limited")
  ) {
    failures.push(
      "Desktop OAuth must use the strict no-redirect credentialed client and bounded response readers.",
    );
  }

  if (
    !googleOAuthProduction.includes("parse_callback_request") ||
    !googleOAuthProduction.includes("OAUTH_CALLBACK_IO_TIMEOUT") ||
    !googleOAuthProduction.includes("let callback_loop = async") ||
    !googleOAuthProduction.includes("400 Bad Request")
  ) {
    failures.push(
      "Google OAuth must keep listening after malformed localhost callbacks and bound each callback connection.",
    );
  }

  if (
    !githubOAuthProduction.includes('verification_uri.path() == "/login/device"') ||
    !githubOAuthProduction.includes('verification_uri.host_str() == Some("github.com")')
  ) {
    failures.push(
      "GitHub OAuth must validate the provider-supplied verification URL before opening it.",
    );
  }

  return failures;
}
