const AGENT_MODULE = "apps/desktop/src-tauri/crates/engine/src/agent.rs";
const DESKTOP_CONSTANTS = "apps/desktop/src-tauri/src/constants.rs";
const RUST_ROOTS = ["apps/desktop/src-tauri/src", "apps/desktop/src-tauri/crates"];

// A literal product identity: `SiteCMD/1.5.4`, `SiteCMD/0.1`. The engine's own
// module is where the format is authored and tested.
const HARDCODED_IDENTITY = /"SiteCMD\/[0-9]/;

export function scannerIdentityFailures(read, listFiles) {
  const failures = [];

  const agent = read(AGENT_MODULE);
  if (!agent.includes('pub const SCANNER_DOCS_URL: &str = "https://sitecmd.com/scanner";')) {
    failures.push(
      `${AGENT_MODULE} must keep SCANNER_DOCS_URL pointing at the page that documents the bot; the URL rides in the User-Agent so operators can look the traffic up.`,
    );
  }
  if (!/pub fn user_agent\(version: &str\) -> String/.test(agent)) {
    failures.push(
      `${AGENT_MODULE} must build the User-Agent from an injected version, so the desktop, the CLI, and the hosted runner cannot identify themselves differently.`,
    );
  }

  const constants = read(DESKTOP_CONSTANTS);
  if (
    !/USER_AGENT[\s\S]{0,200}sitecmd_engine::agent::user_agent\(env!\("CARGO_PKG_VERSION"\)\)/.test(
      constants,
    )
  ) {
    failures.push(
      `${DESKTOP_CONSTANTS}: USER_AGENT must come from sitecmd_engine::agent::user_agent(env!("CARGO_PKG_VERSION")) so it can never go stale against the shipped release again.`,
    );
  }

  const offenders = RUST_ROOTS.flatMap((root) => listFiles(root, (f) => f.endsWith(".rs"))).filter(
    (file) => file !== AGENT_MODULE && HARDCODED_IDENTITY.test(read(file)),
  );
  if (offenders.length > 0) {
    failures.push(
      `Only ${AGENT_MODULE} may write a versioned SiteCMD identity literal; every request builds its User-Agent from crate::constants::USER_AGENT: ${offenders.join(", ")}`,
    );
  }

  return failures;
}
