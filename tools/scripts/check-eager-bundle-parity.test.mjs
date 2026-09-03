import { describe, expect, it } from "vitest";
import {
  collectEagerAssets,
  eagerBundleParityFailures,
  eagerSizeFailures,
  htmlAssetSeeds,
  matchesAssetGlob,
  resolveAssetPath,
} from "./check-eager-bundle-parity.mjs";

// The shape a real build emits: index.html hand-links an unhashed stylesheet
// Vite copied from public/, then names the entry chunk, its preloads, and the
// entry stylesheet under assets/. The bootstrap App chunk reaches further
// assets only through its own static import list.
const html = [
  '<link rel="icon" type="image/svg+xml" href="/favicon.svg">',
  '<link rel="stylesheet" href="/boot.css">',
  '<script type="module" src="/assets/index-abc.js"></script>',
  '<link rel="stylesheet" href="/assets/index-abc.css">',
].join("");
const mainSource = 'const { default: App } = await import("./App");';

const assets = {
  "boot.css": ".startup{color:black}",
  "assets/index-abc.js": 'import"./rolldown-runtime-abc.js";',
  "assets/index-abc.css": ".app{color:red}",
  "assets/rolldown-runtime-abc.js": "export const runtime = 1;",
  "assets/App-def.js": [
    'import{a}from"./index-abc.js";',
    'import*as b from"./shared-jkl.js";',
    'export{c}from"./severity-mno.js";',
    'import"./panel-pqr.css";',
    'const load=()=>import("./IssuesPage-ghi.js");',
  ].join("\n"),
  "assets/shared-jkl.js": 'import"./deep-stu.js";',
  "assets/deep-stu.js": "export const deep = 1;",
  "assets/severity-mno.js": "export const c = 1;",
  "assets/panel-pqr.css": '@import "./theme-yz.css";.panel{color:blue}',
  "assets/theme-yz.css": ".theme{color:green}",
  "assets/IssuesPage-ghi.js": 'import{d}from"./lazy-only-vwx.js";',
  "assets/lazy-only-vwx.js": "export const d = 1;",
};

const eagerBudget = [
  "dist/boot.css",
  "dist/assets/index-*.js",
  "dist/assets/rolldown-runtime-*.js",
  "dist/assets/App-*.js",
  "dist/assets/shared-*.js",
  "dist/assets/deep-*.js",
  "dist/assets/severity-*.js",
  "dist/assets/panel-*.css",
  "dist/assets/theme-*.css",
  "dist/assets/index-*.css",
];

describe("matchesAssetGlob", () => {
  it("matches size-limit globs against dist-relative asset paths", () => {
    expect(matchesAssetGlob("dist/assets/App-*.js", "assets/App-def.js")).toBe(true);
    expect(matchesAssetGlob("dist/assets/App-*.js", "assets/AppRoutes-def.js")).toBe(false);
    expect(matchesAssetGlob("dist/boot.css", "boot.css")).toBe(true);
    // `*` stops at a path separator, so an assets/ glob cannot reach dist root.
    expect(matchesAssetGlob("dist/assets/*.css", "boot.css")).toBe(false);
  });
});

describe("resolveAssetPath", () => {
  it("resolves relative and root-absolute references against the build", () => {
    expect(resolveAssetPath("assets/App-def.js", "./shared-jkl.js")).toBe("assets/shared-jkl.js");
    expect(resolveAssetPath("index.html", "/boot.css")).toBe("boot.css");
    expect(resolveAssetPath("index.html", "/assets/index-abc.js")).toBe("assets/index-abc.js");
  });

  it("rejects references that leave the build", () => {
    expect(resolveAssetPath("index.html", "https://cdn.example.com/x.js")).toBeNull();
    expect(resolveAssetPath("assets/App-def.js", "../../outside.js")).toBeNull();
  });
});

describe("htmlAssetSeeds", () => {
  it("seeds every JS and CSS file index.html loads, inside assets/ or not", () => {
    expect(htmlAssetSeeds(html).sort()).toEqual([
      "assets/index-abc.css",
      "assets/index-abc.js",
      "boot.css",
    ]);
  });
});

describe("collectEagerAssets", () => {
  it("counts a render-blocking stylesheet index.html links outside assets/", () => {
    // dist/boot.css is copied unhashed from public/ and blocks the first paint.
    expect(collectEagerAssets({ html, assets })).toContain("boot.css");
  });

  it("follows chunks reachable only through another chunk's import list", () => {
    const eager = collectEagerAssets({ html, assets });
    // shared-jkl is named by no HTML tag; only App-def.js statically imports it,
    // and deep-stu.js only shared-jkl.js does.
    expect(eager).toContain("assets/shared-jkl.js");
    expect(eager).toContain("assets/deep-stu.js");
    expect(eager).toContain("assets/severity-mno.js");
  });

  it("follows CSS reachable only through a chunk, and CSS it imports in turn", () => {
    const eager = collectEagerAssets({ html, assets });
    expect(eager).toContain("assets/panel-pqr.css");
    expect(eager).toContain("assets/theme-yz.css");
  });

  it("does not follow dynamic imports", () => {
    const eager = collectEagerAssets({ html, assets });
    expect(eager).not.toContain("assets/IssuesPage-ghi.js");
    expect(eager).not.toContain("assets/lazy-only-vwx.js");
  });

  it("reports the complete eager graph and nothing else", () => {
    expect(collectEagerAssets({ html, assets })).toEqual([
      "assets/App-def.js",
      "assets/deep-stu.js",
      "assets/index-abc.css",
      "assets/index-abc.js",
      "assets/panel-pqr.css",
      "assets/rolldown-runtime-abc.js",
      "assets/severity-mno.js",
      "assets/shared-jkl.js",
      "assets/theme-yz.css",
      "boot.css",
    ]);
  });
});

describe("eagerBundleParityFailures", () => {
  it("counts the bootstrap App chunk and the stylesheet as eager", () => {
    const failures = eagerBundleParityFailures({
      html,
      mainSource,
      assets,
      budgetPaths: ["dist/assets/index-*.js"],
    });
    expect(failures.join("\n")).toContain("assets/App-def.js");
    expect(failures.join("\n")).toContain("assets/index-abc.css");
  });

  it("flags an eager file outside assets/ that the budget misses", () => {
    const failures = eagerBundleParityFailures({
      html,
      mainSource,
      assets,
      budgetPaths: eagerBudget.filter((glob) => glob !== "dist/boot.css"),
    });
    expect(failures.join("\n")).toContain("boot.css");
  });

  it("flags transitive eager assets the budget misses", () => {
    const failures = eagerBundleParityFailures({
      html,
      mainSource,
      assets,
      budgetPaths: eagerBudget.filter(
        (glob) => glob !== "dist/assets/shared-*.js" && glob !== "dist/assets/panel-*.css",
      ),
    });
    expect(failures.join("\n")).toContain("assets/shared-jkl.js");
    expect(failures.join("\n")).toContain("assets/panel-pqr.css");
  });

  it("flags lazy chunks that the budget counts", () => {
    const failures = eagerBundleParityFailures({
      html,
      mainSource,
      assets,
      budgetPaths: [...eagerBudget, "dist/assets/IssuesPage-*.js"],
    });
    expect(failures.join("\n")).toContain("assets/IssuesPage-ghi.js");
  });

  it("passes when the budget lists exactly the eager assets", () => {
    expect(
      eagerBundleParityFailures({ html, mainSource, assets, budgetPaths: eagerBudget }),
    ).toEqual([]);
  });

  it('fails loudly if main.tsx stops booting through import("./App")', () => {
    const failures = eagerBundleParityFailures({
      html,
      mainSource: "createRoot(root).render(<App />)",
      assets,
      budgetPaths: eagerBudget,
    });
    expect(failures.join("\n")).toContain("BOOTSTRAP_CHUNK");
  });
});

describe("eagerSizeFailures", () => {
  const eager = ["assets/App-def.js", "boot.css"];

  it("fails instead of skipping when the limit cannot be parsed", () => {
    // A skipped size check would print a pass line for an unmeasured graph,
    // which is the false pass this gate exists to prevent.
    for (const limit of ["lots", "206", "206 gigglebytes", "", undefined, null]) {
      const failures = eagerSizeFailures({ limit, eagerBytes: 1000, eager });
      expect(failures, `limit ${JSON.stringify(limit)} must fail`).toHaveLength(1);
      expect(failures[0]).toContain("cannot parse");
      expect(failures[0]).toContain("never checked against a budget");
    }
  });

  it("fails when the eager graph is over the budget", () => {
    const failures = eagerSizeFailures({ limit: "206 kB", eagerBytes: 206_001, eager });
    expect(failures.join("\n")).toContain("over the 206 kB initial-page budget");
    expect(failures.join("\n")).toContain("assets/App-def.js");
  });

  it("passes when the eager graph fits the budget", () => {
    expect(eagerSizeFailures({ limit: "206 kB", eagerBytes: 206_000, eager })).toEqual([]);
    expect(eagerSizeFailures({ limit: "1 MB", eagerBytes: 999_999, eager })).toEqual([]);
  });
});
