// Match only high-signal secret parameters interpolated into Rust format strings.
const URL_SECRET_RE = /[?&](api_?key|access_token|client_secret|secret)=\{/i;

const INTEGRATIONS_DIR = "apps/desktop/src-tauri/src/integrations";

export function integrationUrlSecretFailures(read, exists, listFiles) {
  if (!exists(INTEGRATIONS_DIR)) return [];

  const failures = [];
  for (const file of listFiles(INTEGRATIONS_DIR, (f) => f.endsWith(".rs"))) {
    const lines = read(file).split("\n");
    for (let i = 0; i < lines.length; i += 1) {
      if (URL_SECRET_RE.test(lines[i])) {
        failures.push(
          `${file}:${i + 1} - embeds a credential in a URL format string; reqwest's error Display leaks the URL into logs. Pass it via .query(&[(...)]) and format reqwest errors with .without_url() (see integrations/pagespeed.rs). Line: ${lines[i].trim()}`,
        );
      }
    }
  }
  return failures;
}
