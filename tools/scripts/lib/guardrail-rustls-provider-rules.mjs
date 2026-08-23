export function rustlsCryptoProviderFailures(read, listFiles) {
  const failures = [];

  const rustFiles = listFiles(
    "apps/desktop/src-tauri/src",
    (file) => file.endsWith(".rs") && !/[/\\]tests?[/\\]/.test(file),
  );

  // This matches only provider-less `builder()` calls after comments are
  // stripped, not `builder_with_provider(`.
  const stripLineComments = (src) => src.replace(/\/\/.*$/gm, "");
  const offenders = rustFiles.filter((file) =>
    /ClientConfig::builder\(\)/.test(stripLineComments(read(file))),
  );
  if (offenders.length > 0) {
    failures.push(
      `rustls ClientConfig must bind a crypto provider via builder_with_provider (the provider-less ClientConfig::builder() panics in headless/CLI scans where no process-default provider is installed, silently dropping the SSL + timing checks): ${offenders.join(", ")}`,
    );
  }

  const probe = read("apps/desktop/src-tauri/src/ssl_probe.rs");
  if (
    !probe.includes("fn platform_verified_client_config") ||
    !probe.includes("builder_with_provider") ||
    !probe.includes("crypto::ring::default_provider")
  ) {
    failures.push(
      "ssl_probe.rs must expose platform_verified_client_config() that binds the ring provider via builder_with_provider so headless scans never depend on a process-default crypto provider",
    );
  }

  return failures;
}
