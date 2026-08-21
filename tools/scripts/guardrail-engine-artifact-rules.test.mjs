import { describe, expect, it } from "vitest";

import { engineArtifactFailures } from "./lib/guardrail-engine-artifact-rules.mjs";

const WASM_LIB = "apps/desktop/src-tauri/crates/engine-wasm/src/lib.rs";
const WASM_MANIFEST = "apps/desktop/src-tauri/crates/engine-wasm/Cargo.toml";
const RUNNERS = "apps/desktop/src-tauri/crates/engine/src/evaluation/runners.rs";
const PROBE_CHECKS = "apps/desktop/src-tauri/crates/engine/src/evaluation/probe_checks.rs";
const PROBE_SCHEMA = "apps/desktop/src-tauri/crates/engine/src/evaluation/probes.rs";
const SCHEMA = "apps/desktop/src-tauri/crates/engine/src/evaluation/mod.rs";
const GOLDEN_TEST = "apps/desktop/src-tauri/crates/engine/tests/golden_checks.rs";
const MANIFEST = "apps/desktop/src-tauri/crates/engine/manifest/capability_manifest.json";
const CHECKS_SCRIPT = "tools/scripts/build-checks-wasm.mjs";
const SCORER_SCRIPT = "tools/scripts/build-scorer-wasm.mjs";

const gated = (symbol) =>
  [
    '#[cfg(feature = "checks")]',
    "#[no_mangle]",
    `pub unsafe extern "C" fn ${symbol}(ptr: *mut u8, len: u32) -> *const u8 {}`,
  ].join("\n");

const WASM_LIB_BASE = [
  'pub extern "C" fn scorer_alloc(len: u32) -> *mut u8 {}',
  'pub unsafe extern "C" fn scorer_free(ptr: *mut u8, len: u32) {}',
  'pub unsafe extern "C" fn scorer_score(ptr: *mut u8, len: u32) -> *const u8 {}',
  gated("engine_evaluate"),
  gated("engine_probe_plan"),
].join("\n\n");

const WASM_MANIFEST_BASE = ["[features]", "default = []", "checks = []", ""].join("\n");

const SCHEMA_BASE = [
  "pub struct EvaluationRequest {",
  "    pub page: PageArtifact,",
  "}",
  "pub struct EvaluationResponse {",
  "    pub planned: Vec<PlannedCheck>,",
  "}",
  "pub enum NotEvaluatedReason {",
  "    MissingFact { fact: RuntimeFact },",
  "}",
].join("\n");

const RUNNERS_BASE = [
  "pub const EXCLUDED_ARTIFACT_CHECKS: &[(&str, &str)] = &[",
  '    ("seo.headings.h1", "HeadingCheck is unregistered on the desktop"),',
  "];",
  "pub const RUNNERS: &[Runner] = &[",
  "    Runner {",
  '        covers: &["security.mixed_content"],',
  "        run: |inputs| MixedContentCheck.run(inputs.page),",
  "    },",
  "    Runner {",
  '        covers: &["security.headers.cross_origin"],',
  "        run: |inputs| {",
  "            crate::checks::security::cross_origin::CrossOriginIsolationCheck.run(inputs.page)",
  "        },",
  "    },",
  "    Runner {",
  '        covers: &["performance.page_weight"],',
  "        run: |inputs| vec![html_size_result(inputs.page.body.len())],",
  "    },",
  "];",
].join("\n");

const PROBE_CHECKS_BASE = [
  "pub const PROBE_CHECKS: &[ProbeCheck] = &[",
  "    ProbeCheck {",
  '        covers: &["config.favicon"],',
  "        plan: |ctx| favicon_plan(ctx),",
  "        grade: |ctx| favicon_grade(ctx),",
  "    },",
  "];",
].join("\n");

const PROBE_SCHEMA_BASE = [
  "pub struct PlannedProbe {}",
  "pub struct ExecutedProbe {}",
  "pub struct ProbePlan {}",
  "pub const EXCLUDED_PROBE_CHECKS: &[(&str, &str)] = &[",
  '    ("seo.robots_txt", "the robots.txt fetch is still planned by the desktop shell"),',
  "];",
  "pub fn probe_plan() {}",
].join("\n");

const FETCH = ["page_artifact", "fetch"];
const MANIFEST_BASE = JSON.stringify({
  schema_version: 1,
  manifest_digest: "ad51493a86e8be14",
  entries: [
    { check: "security.mixed_content", hosted: "artifact", requires: ["page_artifact"] },
    { check: "security.headers.cross_origin", hosted: "artifact", requires: ["page_artifact"] },
    { check: "performance.page_weight", hosted: "artifact", requires: ["page_artifact"] },
    { check: "seo.headings.h1", hosted: "artifact", requires: ["page_artifact"] },
    { check: "seo.title", hosted: "unsupported", requires: [] },
    { check: "security.dns.spf", hosted: "probe_adapter", requires: ["resolver"] },
    { check: "config.favicon", hosted: "probe_adapter", requires: FETCH },
    { check: "seo.robots_txt", hosted: "probe_adapter", requires: FETCH },
  ],
});

const GOLDEN_TEST_BASE = 'const CORPUS: &str = include_str!("../fixtures/checks/golden.json");';

const REGISTRATION = "apps/desktop/src-tauri/src/checks/security/mod.rs";
const REGISTRATION_BASE = [
  "pub fn sync_checks() -> Vec<Box<dyn Check>> {",
  "    vec![",
  "        Box::new(mixed_content::MixedContentCheck),",
  "        Box::new(cross_origin::CrossOriginIsolationCheck),",
  "    ]",
  "}",
].join("\n");

const CHECKS_SCRIPT_BASE = [
  'const corpusPath = path.join(cargoRoot, "crates", "engine", "fixtures", "checks", "golden.json");',
  "const dirtySuffix = engineTreeSuffix(allowDirty);",
  "const provenance = {",
  '  crate: "sitecmd-engine-wasm",',
  '  features: ["checks"],',
  "  engine_commit: head + dirtySuffix,",
  "  rustc: rustcVersion,",
  "  target: WASM_TARGET,",
  "  profile: WASM_PROFILE,",
  "  artifact_sha256: sha256(artifact),",
  "  corpus_sha256: sha256(corpus),",
  "  browser_assets_sha256: sha256(browserAssets),",
  "  manifest_digest: manifest.manifest_digest,",
  "};",
  "vendorSet({",
  "  outDir,",
  "  files: [",
  '    { name: "checks.wasm", bytes: artifact },',
  '    { name: "golden.json", bytes: corpus },',
  '    { name: "browser-assets.json", bytes: browserAssets },',
  "  ],",
  "  record: provenance,",
  '  recordName: "checks-artifact.json",',
  "});",
  'const axeCore = readFileSync(path.join(browserPath, "axe.min.js"), "utf8");',
  'const cwvObserver = readFileSync(path.join(browserPath, "cwv_observer.js"), "utf8");',
  'const cwvRead = readFileSync(path.join(browserPath, "cwv_read.js"), "utf8");',
  'const browserPayload = JSON.parse(run("cargo", ["run", "--example", "emit_browser_payload"]));',
  "axe_core_version: browserPayload.axe_core_version,",
  "axe_run_script: browserPayload.axe_run_script,",
].join("\n");

const SCORER_SCRIPT_BASE = [
  "vendorSet({",
  "  outDir,",
  '  files: [{ name: "scorer.wasm", bytes: artifact }],',
  "  record: provenance,",
  '  recordName: "scorer-artifact.json",',
  "});",
].join("\n");

function run(overrides = {}) {
  const fixture = {
    [WASM_LIB]: WASM_LIB_BASE,
    [WASM_MANIFEST]: WASM_MANIFEST_BASE,
    [RUNNERS]: RUNNERS_BASE,
    [PROBE_CHECKS]: PROBE_CHECKS_BASE,
    [PROBE_SCHEMA]: PROBE_SCHEMA_BASE,
    [SCHEMA]: SCHEMA_BASE,
    [GOLDEN_TEST]: GOLDEN_TEST_BASE,
    [MANIFEST]: MANIFEST_BASE,
    [CHECKS_SCRIPT]: CHECKS_SCRIPT_BASE,
    [SCORER_SCRIPT]: SCORER_SCRIPT_BASE,
    [REGISTRATION]: REGISTRATION_BASE,
    ...overrides,
  };
  return engineArtifactFailures((file) => fixture[file] ?? "");
}

const flags = (failures, fragment) => failures.some((failure) => failure.includes(fragment));

describe("engineArtifactFailures", () => {
  it("accepts a tree where every rule holds", () => {
    expect(run()).toEqual([]);
  });

  it("rejects a renamed scorer ABI symbol the connect worker resolves by name", () => {
    const failures = run({
      [WASM_LIB]: WASM_LIB_BASE.replace('extern "C" fn scorer_score(', 'extern "C" fn evaluate('),
    });
    expect(flags(failures, "no longer exports `scorer_score`")).toBe(true);
  });

  it("rejects an evaluate export that is not behind the checks feature", () => {
    const failures = run({
      [WASM_LIB]: WASM_LIB_BASE.replace('#[cfg(feature = "checks")]\n', ""),
    });
    expect(flags(failures, "is not behind")).toBe(true);
  });

  it("rejects a missing evaluate export", () => {
    const failures = run({
      [WASM_LIB]: WASM_LIB_BASE.replace(
        'pub unsafe extern "C" fn engine_evaluate(ptr: *mut u8, len: u32) -> *const u8 {}',
        "",
      ),
    });
    expect(flags(failures, "no longer exports `engine_evaluate`")).toBe(true);
  });

  it("rejects a default feature set that would change connect's vendored artifact", () => {
    const failures = run({
      [WASM_MANIFEST]: WASM_MANIFEST_BASE.replace("default = []", 'default = ["checks"]'),
    });
    expect(flags(failures, "default feature set must stay empty")).toBe(true);
  });

  it("rejects removing the checks feature that separates the two artifacts", () => {
    const failures = run({
      [WASM_MANIFEST]: WASM_MANIFEST_BASE.replace("checks = []", ""),
    });
    expect(flags(failures, "the `checks` feature is gone")).toBe(true);
  });

  it("rejects a request schema redefined inside the wasm wrapper", () => {
    const failures = run({
      [WASM_LIB]: `${WASM_LIB_BASE}\nstruct EvaluationRequest { page: PageArtifact }`,
    });
    expect(flags(failures, "authored in the engine crate")).toBe(true);
  });

  it("rejects an engine crate that lost the not-evaluated vocabulary", () => {
    const failures = run({
      [SCHEMA]: SCHEMA_BASE.replace("pub enum NotEvaluatedReason {", "pub enum Unused {"),
    });
    expect(flags(failures, "NotEvaluatedReason")).toBe(true);
  });

  it("rejects an artifact-lane check with no runner and no documented exclusion", () => {
    const failures = run({
      [RUNNERS]: RUNNERS_BASE.replace('covers: &["performance.page_weight"]', 'covers: &["x.y"]'),
    });
    expect(flags(failures, "performance.page_weight")).toBe(true);
  });

  it("rejects an id claimed by two runners", () => {
    const failures = run({
      [RUNNERS]: RUNNERS_BASE.replace(
        'covers: &["performance.page_weight"]',
        'covers: &["performance.page_weight", "security.mixed_content"]',
      ),
    });
    expect(flags(failures, "claimed by more than one runner")).toBe(true);
  });

  it("rejects an exclusion with no reason", () => {
    const failures = run({
      [RUNNERS]: RUNNERS_BASE.replace(
        '("seo.headings.h1", "HeadingCheck is unregistered on the desktop")',
        '("seo.headings.h1", "")',
      ),
    });
    expect(flags(failures, "carry no reason")).toBe(true);
  });

  it("flags the rules as stale when the runner table is renamed away", () => {
    const failures = run({ [RUNNERS]: "pub const TABLE: &[Runner] = &[];" });
    expect(flags(failures, "update these rules with it")).toBe(true);
  });

  it("rejects vendoring a corpus other than the one the golden test embeds", () => {
    const failures = run({
      [CHECKS_SCRIPT]: CHECKS_SCRIPT_BASE.replace('"fixtures", "checks"', '"generated", "hosted"'),
    });
    expect(flags(failures, "does not vendor the corpus")).toBe(true);
  });

  it("rejects a build script that writes a vendored file directly", () => {
    const failures = run({
      [CHECKS_SCRIPT]: `${CHECKS_SCRIPT_BASE}\nwriteFileSync(path.join(outDir, "extra.json"), "{}");`,
    });
    expect(flags(failures, "writes a vendored file directly")).toBe(true);
  });

  it("rejects a checks artifact that omits the shared browser payloads", () => {
    const failures = run({
      [CHECKS_SCRIPT]: CHECKS_SCRIPT_BASE.replace(
        '{ name: "browser-assets.json", bytes: browserAssets },',
        "",
      ),
    });
    expect(flags(failures, "browser instrumentation payloads")).toBe(true);
  });

  it("rejects a build script that hardcodes the axe version beside the shared payload", () => {
    const failures = run({
      [CHECKS_SCRIPT]: CHECKS_SCRIPT_BASE.replace(
        "axe_core_version: browserPayload.axe_core_version,",
        'axe_core_version: "4.11.2",',
      ),
    });
    expect(flags(failures, "axe version from the shared browser payload")).toBe(true);
  });

  it("rejects a build script that stops vendoring through the atomic writer", () => {
    const failures = run({
      [SCORER_SCRIPT]: SCORER_SCRIPT_BASE.replace("vendorSet(", "emitFiles("),
    });
    expect(flags(failures, "does not vendor through")).toBe(true);
  });

  it("rejects a provenance record missing a digest", () => {
    const failures = run({
      [CHECKS_SCRIPT]: CHECKS_SCRIPT_BASE.replace("  corpus_sha256: sha256(corpus),\n", ""),
    });
    expect(flags(failures, "corpus_sha256")).toBe(true);
  });

  it("rejects a build script that no longer refuses a dirty engine tree", () => {
    const failures = run({
      [CHECKS_SCRIPT]: CHECKS_SCRIPT_BASE.replace(
        "const dirtySuffix = engineTreeSuffix(allowDirty);",
        'const dirtySuffix = "";',
      ),
    });
    expect(flags(failures, "no longer refuses a dirty engine tree")).toBe(true);
  });

  it("rejects a hosted runner for a check no desktop scan registers", () => {
    const failures = run({ [REGISTRATION]: "pub fn sync_checks() { vec![] }" });
    expect(flags(failures, "not registered in any desktop")).toBe(true);
  });

  it("rejects a brace-wrapped hosted runner whose desktop registration was deleted", () => {
    const failures = run({
      [REGISTRATION]: REGISTRATION_BASE.replace(
        "        Box::new(cross_origin::CrossOriginIsolationCheck),\n",
        "",
      ),
    });
    expect(flags(failures, "not registered in any desktop")).toBe(true);
    expect(flags(failures, "CrossOriginIsolationCheck")).toBe(true);
    expect(flags(failures, "MixedContentCheck")).toBe(false);
  });

  it("rejects a runner formatting the extraction cannot read rather than exempting it", () => {
    const failures = run({
      [RUNNERS]: RUNNERS_BASE.replace(
        "        run: |inputs| MixedContentCheck.run(inputs.page),",
        ["        run: |inputs|", "            MixedContentCheck.run(inputs.page),"].join("\n"),
      ),
    });
    expect(flags(failures, "reads 1 of the 2 check invocations")).toBe(true);
  });

  it("does not read a covers list below the table terminator as a claim", () => {
    const failures = run({
      [RUNNERS]: [
        RUNNERS_BASE.replace('covers: &["performance.page_weight"]', 'covers: &["config.robots"]'),
        "",
        "fn page_weight_example() -> Runner {",
        "    // The row this replaced used to read:",
        '    //     covers: &["performance.page_weight"],',
        "    unimplemented!()",
        "}",
      ].join("\n"),
    });
    expect(flags(failures, "performance.page_weight")).toBe(true);
  });

  it("rejects vendoring worker TypeScript through the artifact path", () => {
    const failures = run({
      [CHECKS_SCRIPT]: CHECKS_SCRIPT_BASE.replace(
        '{ name: "golden.json", bytes: corpus },',
        '{ name: "golden.json", bytes: corpus },\n    { name: "csp-check.ts", bytes: source },',
      ),
    });
    expect(flags(failures, "reimplementation the parity contract exists to forbid")).toBe(true);
  });

  it("reports an unparseable capability manifest rather than passing silently", () => {
    const failures = run({ [MANIFEST]: "{ not json" });
    expect(flags(failures, "unparseable")).toBe(true);
  });

  it("rejects a missing probe-plan export", () => {
    const failures = run({
      [WASM_LIB]: WASM_LIB_BASE.replace(gated("engine_probe_plan"), ""),
    });
    expect(flags(failures, "no longer exports `engine_probe_plan`")).toBe(true);
  });

  it("rejects a probe-plan export gated only by the evaluate export's own cfg", () => {
    const failures = run({
      [WASM_LIB]: WASM_LIB_BASE.replace(
        gated("engine_probe_plan"),
        gated("engine_probe_plan").replace('#[cfg(feature = "checks")]\n', ""),
      ),
    });
    expect(flags(failures, "`engine_probe_plan` is not behind")).toBe(true);
    expect(flags(failures, "`engine_evaluate` is not behind")).toBe(false);
  });

  it("rejects a plan schema redefined inside the wasm wrapper", () => {
    const failures = run({
      [WASM_LIB]: `${WASM_LIB_BASE}\n\nstruct ProbePlan { probes: Vec<PlannedProbe> }`,
    });
    expect(flags(failures, "two definitions of one plan")).toBe(true);
  });

  it("rejects an engine crate that lost the executed-probe definition", () => {
    const failures = run({
      [PROBE_SCHEMA]: PROBE_SCHEMA_BASE.replace("pub struct ExecutedProbe {}", ""),
    });
    expect(flags(failures, "struct ExecutedProbe")).toBe(true);
  });

  it("rejects a probe-lane check with no probe check and no documented exclusion", () => {
    const failures = run({
      [PROBE_CHECKS]: PROBE_CHECKS_BASE.replace('covers: &["config.favicon"]', 'covers: &["x.y"]'),
    });
    expect(flags(failures, "config.favicon")).toBe(true);
    expect(flags(failures, "probe-lane checks have no probe check")).toBe(true);
  });

  it("rejects an id claimed by two probe checks", () => {
    const failures = run({
      [PROBE_CHECKS]: PROBE_CHECKS_BASE.replace(
        'covers: &["config.favicon"]',
        'covers: &["config.favicon", "config.favicon"]',
      ),
    });
    expect(flags(failures, "claimed by more than one probe check")).toBe(true);
  });

  it("rejects a probe exclusion with no reason", () => {
    const failures = run({
      [PROBE_SCHEMA]: PROBE_SCHEMA_BASE.replace(
        '"the robots.txt fetch is still planned by the desktop shell"',
        '""',
      ),
    });
    expect(flags(failures, "carry no reason")).toBe(true);
  });

  it("rejects an id that is both planned and excluded", () => {
    const failures = run({
      [PROBE_CHECKS]: PROBE_CHECKS_BASE.replace(
        'covers: &["config.favicon"]',
        'covers: &["config.favicon", "seo.robots_txt"]',
      ),
    });
    expect(flags(failures, "both claimed and excluded")).toBe(true);
  });

  it("flags the rules as stale when the probe table is renamed away", () => {
    const failures = run({ [PROBE_CHECKS]: "pub const TABLE: &[ProbeCheck] = &[];" });
    expect(flags(failures, "update these rules with them")).toBe(true);
  });

  it("does not demand a planner for a probe-lane check no fetch can supply", () => {
    expect(flags(run(), "security.dns.spf")).toBe(false);
  });
});
