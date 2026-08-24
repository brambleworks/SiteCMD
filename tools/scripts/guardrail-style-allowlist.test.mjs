import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relative) => fs.readFileSync(path.join(ROOT, relative), "utf8");

function inlineStyleAllowlist() {
  const source = read("tools/scripts/lib/guardrail-style-rules.mjs");
  const block = source.match(/INLINE_STYLE_ALLOWED_FILES = new Set\(\[([\s\S]*?)\]\)/);
  if (!block) throw new Error("INLINE_STYLE_ALLOWED_FILES not found");
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => path.basename(m[1], ".tsx"));
}

describe("inline style exceptions are documented where the rule is stated", () => {
  const docs = [
    "AGENTS.md",
    "apps/desktop/AGENTS.md",
    "apps/desktop/DESIGN.md",
    "apps/desktop/src/styles/COMPONENT_GUIDE.md",
  ];

  it.each(docs)("%s names every allowlisted component", (doc) => {
    const source = read(doc);
    for (const component of inlineStyleAllowlist()) {
      expect(source, `${doc} must mention ${component}`).toContain(component);
    }
  });
});
