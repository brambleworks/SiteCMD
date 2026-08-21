import { describe, expect, it } from "vitest";
import { rules } from "./guardrail-test-support.mjs";

const { commentQualityFailures, tailwindRemovalFailures } = rules;

describe("tailwindRemovalFailures", () => {
  const base = {
    "apps/desktop/package.json": JSON.stringify({
      dependencies: { clsx: "^2.1.1" },
      devDependencies: { vite: "^8.1.5" },
    }),
    "apps/desktop/vite.config.ts": "export default defineConfig({ plugins: [react()] });\n",
    "apps/desktop/src/index.css":
      '@import "./styles/colors.css";\n@import "./styles/layout.css";\n',
    "apps/desktop/src/styles/colors.css":
      ".bg-card {\n  background-color: var(--card);\n}\n.text-foreground {\n  color: var(--foreground);\n}\n",
    "apps/desktop/src/styles/layout.css":
      ".flex {\n  display: flex;\n}\n.min-w-0 {\n  min-width: 0;\n}\n.stack-snug > * + * {\n  margin-top: 0.5rem;\n}\n",
    "apps/desktop/src/components/Panel.tsx":
      'export const Panel = () => <div className="bg-card flex min-w-0 stack-snug text-foreground" />;\n',
  };
  const run = (overrides = {}) => {
    const fixture = { ...base, ...overrides };
    const read = (file) => fixture[file] ?? "";
    const exists = (file) => Object.hasOwn(fixture, file);
    const listFiles = (dir, predicate) =>
      Object.keys(fixture).filter((file) => file.startsWith(`${dir}/`) && predicate(file));
    return tailwindRemovalFailures(read, listFiles, exists);
  };

  it("accepts a clean de-Tailwinded tree where every utility-shaped class is backed by CSS", () => {
    expect(run()).toEqual([]);
  });

  it("flags utility classes with no backing CSS rule", () => {
    const failures = run({
      "apps/desktop/src/components/Bad.tsx":
        'export const Bad = () => <div className="mt-2 gap-3" />;\n',
    });
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("Bad.tsx");
    expect(failures[0]).toContain("mt-2");
    expect(failures[0]).toContain("gap-3");
  });

  it("flags dead utility tokens in dynamic class maps", () => {
    const failures = run({
      "apps/desktop/src/lib/tone.ts":
        'const COLOR_MAP: Record<string, string> = Object.freeze({ quiet: "bg-muted/40" });\nexport const tone = COLOR_MAP.quiet;\n',
    });
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("bg-muted/40");
  });

  it("flags a reintroduced Tailwind dependency", () => {
    const failures = run({
      "apps/desktop/package.json": JSON.stringify({
        dependencies: { clsx: "^2.1.1", tailwindcss: "^4.1.13" },
      }),
    });
    expect(failures.some((f) => f.includes("tailwindcss"))).toBe(true);
  });

  it("flags an @import of tailwindcss in the src/index.css entrypoint", () => {
    const failures = run({
      "apps/desktop/src/index.css": '@import "tailwindcss";\n@import "./styles/colors.css";\n',
    });
    expect(failures.some((f) => f.includes("index.css"))).toBe(true);
  });

  it("flags the @tailwindcss/vite plugin in the Vite config", () => {
    const failures = run({
      "apps/desktop/vite.config.ts":
        'import tailwindcss from "@tailwindcss/vite";\nexport default defineConfig({ plugins: [react(), tailwindcss()] });\n',
    });
    expect(failures.some((f) => f.includes("vite.config.ts"))).toBe(true);
  });
});

describe("commentQualityFailures", () => {
  const run = (fixture) => {
    const read = (file) => fixture[file] ?? "";
    const listFiles = (dir, predicate) =>
      Object.keys(fixture).filter((file) => file.startsWith(`${dir}/`) && predicate(file));
    return commentQualityFailures(read, listFiles);
  };

  it("accepts comments that state things plainly", () => {
    const failures = run({
      "apps/desktop/src/lib/clean.ts":
        "// Cache the parsed value; the raw payload is re-read every tick.\nexport const x = 1;\n",
    });
    expect(failures).toEqual([]);
  });

  it("flags a filler word in a line comment", () => {
    const failures = run({
      "apps/desktop/src/lib/bad.ts": "// This is simply a passthrough.\nexport const y = 2;\n",
    });
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("bad.ts:1");
    expect(failures[0]).toContain("simply");
  });

  it("flags filler in a block comment and reports the physical line", () => {
    const failures = run({
      "apps/desktop/src/lib/block.ts":
        "/**\n * A parser.\n *\n * Note that empty input yields an empty result.\n */\nexport const z = 3;\n",
    });
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("block.ts:4");
    expect(failures[0]).toContain("note that");
  });

  it("ignores filler words inside string literals and JSX text", () => {
    const failures = run({
      "apps/desktop/src/lib/copy.ts": 'export const msg = "Simply enter your key.";\n',
      "apps/desktop/src/components/Note.tsx":
        "export const Note = () => <p>Basically done, of course.</p>;\n",
    });
    expect(failures).toEqual([]);
  });

  it("does not match well / lets / Let's (the false-positive trap)", () => {
    const failures = run({
      "apps/desktop/src-tauri/src/probe.rs":
        "/// Returns a well-formed response; the cap lets callers retry.\n/// Healthy Let's Encrypt certs renew early.\npub fn probe() {}\n",
    });
    expect(failures).toEqual([]);
  });

  it("does not treat // inside a Rust raw string as a comment", () => {
    const failures = run({
      "apps/desktop/src-tauri/src/raw.rs":
        'let url = r#"https://example.test//path basically"#;\n// A real comment is clean.\n',
    });
    expect(failures).toEqual([]);
  });

  it("respects the allow-comment-phrase marker", () => {
    const failures = run({
      "apps/desktop/src-tauri/src/allowed.rs":
        '/// The word "simply" is the subject here. allow-comment-phrase\npub fn f() {}\n',
    });
    expect(failures).toEqual([]);
  });

  it("flags a comment that cites a maintainer-only design tool", () => {
    const failures = run({
      "apps/desktop/src/components/dashboard/DeploysPage.tsx":
        '/**\n * Deploys - Stitch "Kinetic Console" style.\n */\nexport const D = () => null;\n',
      "apps/desktop/src/components/ui/markdown.tsx":
        "// Styled to match the dark Stitch theme.\nexport const M = () => null;\n",
    });
    expect(failures).toHaveLength(2);
    expect(failures.every((f) => f.includes("cites Stitch"))).toBe(true);
    expect(failures.some((f) => f.includes("apps/desktop/DESIGN.md"))).toBe(true);
  });

  it("does not flag 'stitch' used as an ordinary verb", () => {
    const failures = run({
      "apps/desktop/src-tauri/src/scoring/calculator.rs":
        "/// Critical plus a confirmed weaker sibling never stitch into a\n/// floor-piercing score.\npub fn score() {}\n",
    });
    expect(failures).toEqual([]);
  });

  it("scans .rs doc comments and .css block comments", () => {
    const failures = run({
      "apps/desktop/src/styles/x.css":
        "/* Here we recess the control. */\n.x {\n  color: red;\n}\n",
      "apps/desktop/src-tauri/src/y.rs": "//! Now we boot the loop.\npub fn y() {}\n",
    });
    expect(failures).toHaveLength(2);
    expect(failures.some((f) => f.includes("x.css") && f.includes("here we"))).toBe(true);
    expect(failures.some((f) => f.includes("y.rs") && f.includes("now we"))).toBe(true);
  });
});
