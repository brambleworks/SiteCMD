export function tauriCspSafetyFailures(read, exists) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const tauriConfig = read("apps/desktop/src-tauri/tauri.conf.json");
  const cargoManifest = read("apps/desktop/src-tauri/Cargo.toml");
  const desktopPackage = read("apps/desktop/package.json");
  const defaultCapability = read("apps/desktop/src-tauri/capabilities/default.json");
  const openUrlHelper = read("apps/desktop/src/lib/open-url.ts");

  // Inline style attributes would reopen an unnecessary DOM injection path.
  check(
    /default-src 'self'/.test(tauriConfig) &&
      /script-src 'self'/.test(tauriConfig) &&
      !/style-src-attr\s+'unsafe-inline'/.test(tauriConfig),
    "Tauri CSP must keep self-only default/script sources and must not allow style-src-attr 'unsafe-inline'.",
  );
  check(
    /connect-src 'self' ipc: tauri:;/.test(tauriConfig) &&
      !/connect-src[^;]*(?:https?:|wss?:|\*)/.test(tauriConfig),
    "Production Tauri CSP must keep renderer connect-src limited to self/IPC; external telemetry and API traffic belongs behind Rust allowlist brokers.",
  );
  check(
    !/tauri\s*=\s*\{[^}\n]*features\s*=\s*\[[^\]]*"devtools"/m.test(cargoManifest),
    "Production Tauri dependencies must not compile the devtools feature; debug builds already expose the inspector through debug_assertions.",
  );
  check(
    !defaultCapability.includes("opener:") &&
      !cargoManifest.includes("tauri-plugin-opener") &&
      !desktopPackage.includes("@tauri-apps/plugin-opener") &&
      openUrlHelper.includes("await openExternalUrl({ url: safeUrl })") &&
      openUrlHelper.includes('if (import.meta.env.DEV || import.meta.env.MODE === "test")'),
    "Tauri main renderer must not have a generic URL opener; production external links must cross the native-confirmed broker without a browser fallback.",
  );

  // Production must not bypass the native-confirmed URL broker.
  check(
    (openUrlHelper.match(/window\.open/g) || []).length <= 1,
    "open-url.ts must contain at most one window.open (the DEV/test fallback); a second occurrence is a production browser egress that bypasses the native-confirmed broker.",
  );
  check(
    /throw new Error\(/.test(openUrlHelper),
    "open-url.ts must throw for production external links outside the Tauri boundary; without the terminal throw a non-Tauri build would silently skip the native-confirmed broker.",
  );

  // Dev overlays must narrow or extend CSP, never disable it.
  for (const devConfig of [
    "apps/desktop/src-tauri/tauri.attach.conf.json",
    "apps/desktop/src-tauri/tauri.dev.conf.json",
  ]) {
    if (!exists(devConfig)) continue;
    check(
      !/"csp"\s*:\s*null/.test(read(devConfig)),
      `${devConfig} must not set "csp": null (that disables CSP for the dev/attach build); use a dev-scoped CSP instead.`,
    );
  }

  return failures;
}
