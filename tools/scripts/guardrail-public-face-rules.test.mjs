import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { publicFaceFailures } from "./lib/guardrail-public-face-rules.mjs";
import { ROOT, realRead } from "./guardrail-test-support.mjs";

const DOCS_INDEX = "docs/README.md";
const CONTRIBUTING = "CONTRIBUTING.md";
const CONNECTED_SPECS = "docs/engineering/connected-service";
const PRODUCT_DOCS = "docs/product";
const LOCALHOST_FIXTURES = "apps/desktop/src-tauri/src/core/localhost.rs";
const PROTOCOL_SPEC = `${CONNECTED_SPECS}/connected-protocol-spec.md`;

function realFiles() {
  const files = {
    [DOCS_INDEX]: realRead(DOCS_INDEX),
    [CONTRIBUTING]: realRead(CONTRIBUTING),
    [LOCALHOST_FIXTURES]: realRead(LOCALHOST_FIXTURES),
  };
  for (const name of fs.readdirSync(path.join(ROOT, CONNECTED_SPECS))) {
    if (name.endsWith(".md"))
      files[`${CONNECTED_SPECS}/${name}`] = realRead(`${CONNECTED_SPECS}/${name}`);
  }
  for (const name of fs.readdirSync(path.join(ROOT, PRODUCT_DOCS))) {
    if (name.endsWith(".md"))
      files[`${PRODUCT_DOCS}/${name}`] = realRead(`${PRODUCT_DOCS}/${name}`);
  }
  return files;
}

function run(mutate = () => {}) {
  const files = realFiles();
  mutate(files);
  const read = (file) => {
    if (!(file in files)) throw new Error(`no fixture for ${file}`);
    return files[file];
  };
  const exists = (file) => file in files;
  const listFiles = (dir, predicate) =>
    Object.keys(files).filter((file) => file.startsWith(`${dir}/`) && predicate(file));
  return publicFaceFailures(read, exists, listFiles).join("\n");
}

describe("publicFaceFailures", () => {
  it("passes the real repository", () => {
    expect(run()).toBe("");
  });

  // docs/product/get-value-in-5-minutes.md still reads as product intent ("SiteCMD
  // should ...") today; the rule module excludes it until the desktop-repo-public-face
  // plan's Task 8 rewrites the walkthrough. Prove the exclusion is narrow: the intent
  // check still fires for every other product doc.
  it("still catches product intent in a doc other than the pending walkthrough", () => {
    expect(
      run((files) => {
        files["docs/product/fix-your-first-issue.md"] += "\nSiteCMD should do this.\n";
      }),
    ).toContain("reads as product intent");
  });

  it("rejects a docs index that still lists the executed cutover as an entry point", () => {
    expect(
      run((files) => {
        files[DOCS_INDEX] = files[DOCS_INDEX].replace(/ \(historical:[^)]*\)/, "");
      }),
    ).toContain("historical");
  });

  it("rejects public docs that call the committed connected specs private", () => {
    expect(
      run((files) => {
        files[CONTRIBUTING] = "Connected-service internals live in SiteCMD-Web.\n";
      }),
    ).toContain("docs/engineering/connected-service/");
  });

  it("rejects a spec that defers to the private commercial record", () => {
    expect(
      run((files) => {
        files[PROTOCOL_SPEC] +=
          "\nThe billable unit (commercial terms spec) is the connected production site.\n";
      }),
    ).toContain("defers to a private record");
  });

  it("rejects a real third-party domain in the environment fixtures", () => {
    expect(
      run((files) => {
        files[LOCALHOST_FIXTURES] += '("https://upstage.ai", "production"),\n';
      }),
    ).toContain("real third-party domain");
  });

  it("rejects a real tunnelling host the message already claimed to cover", () => {
    expect(
      run((files) => {
        files[LOCALHOST_FIXTURES] += '("https://localhost.run", "production"),\n';
      }),
    ).toContain("real third-party domain");
  });
});
