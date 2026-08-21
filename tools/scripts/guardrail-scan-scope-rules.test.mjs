import { describe, expect, it } from "vitest";
import { scanScopeFailures } from "./lib/guardrail-scan-scope-rules.mjs";

const ENGINE_SCOPE = "apps/desktop/src-tauri/crates/engine/src/scope.rs";
const ENGINE_ROUTE = "apps/desktop/src-tauri/crates/engine/src/route.rs";
const SCHEDULER = "apps/desktop/src-tauri/src/background/scan_scheduler.rs";
const SCOPE_COMMANDS = "apps/desktop/src-tauri/src/commands/scan_scope.rs";
const OVERLAY_STATE = "apps/desktop/src/components/scan/useScanConfigOverlayState.ts";

const HEALTHY = {
  [ENGINE_SCOPE]: `
pub const SCOPE_WIRE_LIMIT: usize = 5_000;
pub const HOSTED_SCOPE_CEILING: usize = 100;
pub struct ScanScope { pub entry_route: String }
pub fn build_scope() {}
`,
  [ENGINE_ROUTE]: "pub const CANONICALIZER_VERSION: u8 = 1;",
  [SCHEDULER]: "let urls = crate::db::scan_scope_urls(db, &url);",
  [SCOPE_COMMANDS]: "let scope = build_scope(&entry, &routes, families, None)?;",
  [OVERLAY_STATE]: `
  const handleStart = useCallback(async () => {
    if (scanType === "code") {
      onStart({ urls: [], axeEnabled: false, scanType });
      return;
    }
    await setScanScope({ siteId, siteUrl, routes });
    onStart({ urls, axeEnabled, scanType });
  }, [siteUrl]);
`,
};

function failuresWith(overrides = {}) {
  const files = { ...HEALTHY, ...overrides };
  return scanScopeFailures((file) => {
    if (!(file in files)) throw new Error(`no fixture for ${file}`);
    return files[file];
  });
}

describe("the two scan paths read one scope", () => {
  it("passes when every rule holds", () => {
    expect(failuresWith()).toEqual([]);
  });

  it("fails when the scheduler stops reading the stored scope", () => {
    const found = failuresWith({
      [SCHEDULER]: "urls: web_focus.map(|_| vec![url.to_string()]).unwrap_or_default(),",
    });
    expect(found.join("\n")).toContain("scan_scope_urls");
  });

  it("fails when the desktop stops building scopes through the engine", () => {
    const found = failuresWith({
      [SCOPE_COMMANDS]: "let stored = routes.clone();",
    });
    expect(found.join("\n")).toContain("build_scope");
  });

  it("fails when a scope is trimmed to fit instead of refused", () => {
    const found = failuresWith({
      [SCOPE_COMMANDS]: "let scope = build_scope(&entry)?;\nroutes.truncate(100);",
    });
    expect(found.join("\n")).toContain("never truncate");
  });

  it("fails when the bounds leave the engine", () => {
    const found = failuresWith({
      [ENGINE_SCOPE]: "pub fn build_scope() {}\npub struct ScanScope { pub entry_route: String }",
    });
    expect(found.join("\n")).toContain("SCOPE_WIRE_LIMIT");
    expect(found.join("\n")).toContain("HOSTED_SCOPE_CEILING");
  });

  it("fails when the scope stops carrying its entry route", () => {
    const found = failuresWith({
      [ENGINE_SCOPE]:
        "pub const SCOPE_WIRE_LIMIT: usize = 5_000;\npub const HOSTED_SCOPE_CEILING: usize = 100;\npub fn build_scope() {}",
    });
    expect(found.join("\n")).toContain("entry route");
  });

  it("fails when the canonicalizer loses its version", () => {
    const found = failuresWith({ [ENGINE_ROUTE]: "pub fn canonical_path() {}" });
    expect(found.join("\n")).toContain("CANONICALIZER_VERSION");
  });
});

describe("the authoring surface", () => {
  it("fails when the selection is never recorded", () => {
    const found = failuresWith({
      [OVERLAY_STATE]: "const handleStart = useCallback(() => { onStart({ urls }); }, [siteUrl]);",
    });
    expect(found.join("\n")).toContain("scan scope");
  });

  it("fails when the run dispatches before the scope is written", () => {
    const found = failuresWith({
      [OVERLAY_STATE]: `
  const handleStart = useCallback(async () => {
    onStart({ urls, axeEnabled, scanType });
    await setScanScope({ siteId, siteUrl, routes });
  }, [siteUrl]);
`,
    });
    expect(found.join("\n")).toContain("BEFORE dispatching");
  });
});
