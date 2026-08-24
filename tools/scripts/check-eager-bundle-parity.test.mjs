import { describe, expect, it } from "vitest";
import { eagerBundleParityFailures, matchesAssetGlob } from "./check-eager-bundle-parity.mjs";

const html =
  '<script type="module" src="/assets/index-abc.js"></script><link rel="stylesheet" href="/assets/index-abc.css">';
const mainSource = 'const { default: App } = await import("./App");';
const assets = ["index-abc.js", "index-abc.css", "App-def.js", "IssuesPage-ghi.js"];

describe("eagerBundleParityFailures", () => {
  it("matches size-limit globs against bare asset names", () => {
    expect(matchesAssetGlob("dist/assets/App-*.js", "App-def.js")).toBe(true);
    expect(matchesAssetGlob("dist/assets/App-*.js", "AppRoutes-def.js")).toBe(false);
  });

  it("counts the bootstrap App chunk and the stylesheet as eager", () => {
    const budgetPaths = ["dist/assets/index-*.js"];
    const failures = eagerBundleParityFailures({ html, mainSource, assets, budgetPaths });
    expect(failures.join("\n")).toContain("App-def.js");
    expect(failures.join("\n")).toContain("index-abc.css");
  });

  it("flags lazy chunks that the budget counts", () => {
    const budgetPaths = ["dist/assets/*.js", "dist/assets/index-*.css"];
    const failures = eagerBundleParityFailures({ html, mainSource, assets, budgetPaths });
    expect(failures.join("\n")).toContain("IssuesPage-ghi.js");
  });

  it("passes when the budget lists exactly the eager assets", () => {
    const budgetPaths = [
      "dist/assets/index-*.js",
      "dist/assets/App-*.js",
      "dist/assets/index-*.css",
    ];
    expect(eagerBundleParityFailures({ html, mainSource, assets, budgetPaths })).toEqual([]);
  });

  it('fails loudly if main.tsx stops booting through import("./App")', () => {
    const failures = eagerBundleParityFailures({
      html,
      mainSource: "createRoot(root).render(<App />)",
      assets,
      budgetPaths: ["dist/assets/index-*.js", "dist/assets/App-*.js", "dist/assets/index-*.css"],
    });
    expect(failures.join("\n")).toContain("BOOTSTRAP_CHUNK");
  });
});
