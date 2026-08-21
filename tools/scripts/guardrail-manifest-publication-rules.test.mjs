import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { manifestPublicationFailures } from "./lib/guardrail-manifest-publication-rules.mjs";

const RELEASE_WORKFLOW = ".github/workflows/release.yml";
const STANDALONE_WORKFLOW = ".github/workflows/publish-capability-manifest.yml";
const OTHER_WORKFLOW = ".github/workflows/knip.yml";
const PUBLISHER = "tools/scripts/publish-capability-manifest.mjs";
const MANIFEST_TEST = "apps/desktop/src-tauri/crates/engine/tests/capability_manifest.rs";
const MANIFEST_ARTIFACT = "apps/desktop/src-tauri/crates/engine/manifest/capability_manifest.json";

const HEALTHY = {
  [RELEASE_WORKFLOW]: `name: Release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: read

jobs:
  tag-gate:
    runs-on: ubuntu-22.04
    steps:
      - run: echo gate

  preflight:
    needs: tag-gate
    runs-on: ubuntu-22.04
    steps:
      - run: pnpm guardrails:repo

  publish-capability-manifest:
    needs: preflight
    runs-on: ubuntu-22.04
    permissions:
      contents: read
      id-token: write
    steps:
      - run: node ./tools/scripts/publish-capability-manifest.mjs

  prepare-candidate:
    needs: preflight
    runs-on: ubuntu-22.04
    steps:
      - run: echo candidate

  build:
    needs:
      [prepare-candidate, publish-capability-manifest]
    runs-on: ubuntu-22.04
    steps:
      - run: echo build
`,
  [STANDALONE_WORKFLOW]: `name: Publish capability manifest

on:
  push:
    branches:
      - main
    paths:
      - "apps/desktop/src-tauri/crates/engine/manifest/capability_manifest.json"
  workflow_dispatch:

permissions:
  contents: read
  id-token: write

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - run: node ./tools/scripts/publish-capability-manifest.mjs
`,
  [OTHER_WORKFLOW]: `name: knip

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - run: pnpm knip:files
`,
  [PUBLISHER]: `const CONNECT_ORIGIN = "https://connect.sitecmd.com";
const MANIFEST_ROUTE = "/v1/engine-manifests/";
const OIDC_AUDIENCE = CONNECT_ORIGIN;
const REMEDIATION =
  "Regenerate the artifact with \\\`cargo test -p sitecmd-engine --test capability_manifest -- --ignored regenerate\\\`.";
`,
  [MANIFEST_TEST]: `#[test]
fn the_published_document_is_current() {}

#[test]
#[ignore = "regenerates the published manifest; run deliberately"]
fn regenerate() {}
`,
  [MANIFEST_ARTIFACT]: JSON.stringify({
    schema_version: 1,
    manifest_digest: "b5fef8de083e1976",
    entries: [
      { check: "seo.title" },
      { check: "security.cookies" },
      { check: "accessibility.axe.", family: true },
      { check: "security.cookies.", family: true },
    ],
  }),
};

function failuresWith(overrides = {}) {
  const files = { ...HEALTHY, ...overrides };
  const read = (file) => {
    if (!(file in files)) throw new Error(`no fixture for ${file}`);
    return files[file];
  };
  const listFiles = (dir, predicate) =>
    Object.keys(files).filter((file) => file.startsWith(`${dir}/`) && predicate(file));
  return manifestPublicationFailures(read, listFiles);
}

describe("the manifest reaches the registry before anything ships under its digest", () => {
  it("passes when every rule holds", () => {
    expect(failuresWith()).toEqual([]);
  });

  it("fails when the release workflow has no publishing job", () => {
    const found = failuresWith({
      [RELEASE_WORKFLOW]: HEALTHY[RELEASE_WORKFLOW].replace(
        /\n {2}publish-capability-manifest:[\s\S]*?\n\n/,
        "\n",
      ).replace(", publish-capability-manifest", ""),
    });
    expect(found.join("\n")).toContain("must keep a publish-capability-manifest job");
  });

  it("fails when the build job can be reached without publication", () => {
    const found = failuresWith({
      [RELEASE_WORKFLOW]: HEALTHY[RELEASE_WORKFLOW].replace(", publish-capability-manifest", ""),
    });
    expect(found.join("\n")).toContain("transitively unreachable");
  });

  it("still passes when a job is inserted between build and publication", () => {
    const found = failuresWith({
      [RELEASE_WORKFLOW]: HEALTHY[RELEASE_WORKFLOW].replace(
        "  build:\n    needs:\n      [prepare-candidate, publish-capability-manifest]",
        "  stage-artifacts:\n    needs: [prepare-candidate, publish-capability-manifest]\n    runs-on: ubuntu-22.04\n    steps:\n      - run: echo stage\n\n  build:\n    needs: stage-artifacts",
      ),
    });
    expect(found).toEqual([]);
  });

  it("fails when a publishing workflow cannot mint an identity", () => {
    const found = failuresWith({
      [STANDALONE_WORKFLOW]: HEALTHY[STANDALONE_WORKFLOW].replace("\n  id-token: write", ""),
    });
    expect(found.join("\n")).toContain("without id-token: write");
  });

  it("fails when no workflow runs the publisher at all", () => {
    const found = failuresWith({
      [RELEASE_WORKFLOW]: HEALTHY[RELEASE_WORKFLOW].replace(
        "node ./tools/scripts/publish-capability-manifest.mjs",
        "echo skipped",
      ),
      [STANDALONE_WORKFLOW]: HEALTHY[STANDALONE_WORKFLOW].replace(
        "node ./tools/scripts/publish-capability-manifest.mjs",
        "echo skipped",
      ),
    });
    expect(found.join("\n")).toContain("No workflow runs");
  });

  it("fails when the publisher addresses another origin", () => {
    const found = failuresWith({
      [PUBLISHER]: HEALTHY[PUBLISHER].replace(
        "https://connect.sitecmd.com",
        "https://connect.staging.sitecmd.com",
      ),
    });
    expect(found.join("\n")).toContain("must address https://connect.sitecmd.com");
  });

  it("fails when the publisher stops using the registry route", () => {
    const found = failuresWith({
      [PUBLISHER]: HEALTHY[PUBLISHER].replace("/v1/engine-manifests/", "/v1/manifests/"),
    });
    expect(found.join("\n")).toContain("/v1/engine-manifests/");
  });

  it("fails when the publisher mints its token for another audience", () => {
    const found = failuresWith({
      [PUBLISHER]: HEALTHY[PUBLISHER].replace(
        "const OIDC_AUDIENCE = CONNECT_ORIGIN;",
        'const OIDC_AUDIENCE = "https://github.com/brambleworks";',
      ),
    });
    expect(found.join("\n")).toContain("audience");
  });

  it("fails when the publisher names the assertion test as the remedy", () => {
    const found = failuresWith({
      [PUBLISHER]: HEALTHY[PUBLISHER].replace(
        "--test capability_manifest -- --ignored regenerate",
        "capability_manifest",
      ),
    });
    expect(found.join("\n")).toContain("regenerates the artifact");
  });

  it("fails when the regeneration test is renamed out from under the instruction", () => {
    const found = failuresWith({
      [MANIFEST_TEST]: HEALTHY[MANIFEST_TEST].replace("fn regenerate()", "fn rewrite_manifest()"),
    });
    expect(found.join("\n")).toContain("must keep the ignored `regenerate` test");
  });

  it("fails when the standalone workflow stops watching the artifact", () => {
    const found = failuresWith({
      [STANDALONE_WORKFLOW]: HEALTHY[STANDALONE_WORKFLOW].replace(
        "apps/desktop/src-tauri/crates/engine/manifest/capability_manifest.json",
        "apps/desktop/src-tauri/**",
      ),
    });
    expect(found.join("\n")).toContain("must watch");
  });

  it("fails when the standalone workflow groups content-addressed publications", () => {
    const found = failuresWith({
      [STANDALONE_WORKFLOW]: HEALTHY[STANDALONE_WORKFLOW].replace(
        "jobs:\n",
        "concurrency:\n  group: publish-capability-manifest\n  cancel-in-progress: false\n\njobs:\n",
      ),
    });
    expect(found.join("\n")).toContain("must not use concurrency grouping");
  });

  it("passes against the repository as it stands", () => {
    const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
    const read = (file) => fs.readFileSync(path.join(ROOT, file), "utf8");
    const listFiles = (dir, predicate) =>
      fs
        .readdirSync(path.join(ROOT, dir))
        .map((entry) => `${dir}/${entry}`)
        .filter((file) => predicate(file));
    expect(manifestPublicationFailures(read, listFiles)).toEqual([]);
  });
});
