import { describe, expect, it } from "vitest";
import { planScan } from "./scan-planner";

const BASE = {
  activeUrl: "https://example.com",
  activeProjectId: 42,
  projectFolder: "/tmp/project",
};

describe("planScan - code mode", () => {
  it("emits a single code action when project folder is linked", () => {
    const actions = planScan({ ...BASE, mode: "code" });
    expect(actions).toEqual([
      { kind: "code", projectId: 42, folder: "/tmp/project", url: "https://example.com" },
    ]);
  });

  it("emits an error when no project folder is linked", () => {
    const actions = planScan({ ...BASE, mode: "code", projectFolder: null });
    expect(actions).toHaveLength(1);
    expect(actions[0].kind).toBe("error");
    if (actions[0].kind === "error") {
      expect(actions[0].message).toContain("linked project folder");
    }
  });

  it("emits an error when no project is active", () => {
    const actions = planScan({ ...BASE, mode: "code", activeProjectId: null });
    expect(actions[0].kind).toBe("error");
  });

  it("never runs a web scan in code mode", () => {
    const actions = planScan({ ...BASE, mode: "code" });
    expect(actions.some((a) => a.kind === "web-single" || a.kind === "web-multi")).toBe(false);
  });
});

describe("planScan - web mode", () => {
  it("emits web-single with the active URL when no URLs are provided", () => {
    const actions = planScan({ ...BASE, mode: "web" });
    expect(actions).toEqual([
      { kind: "web-single", url: "https://example.com", scanType: "health", axeEnabled: false },
    ]);
  });

  it("emits web-multi when multiple URLs are provided", () => {
    const actions = planScan({
      ...BASE,
      mode: "web",
      urls: ["https://example.com/a", "https://example.com/b", "https://example.com/c"],
    });
    expect(actions).toHaveLength(1);
    expect(actions[0]).toMatchObject({
      kind: "web-multi",
      urls: ["https://example.com/a", "https://example.com/b", "https://example.com/c"],
      scanType: "health",
    });
  });

  it("propagates the axeEnabled flag", () => {
    const actions = planScan({ ...BASE, mode: "web", axeEnabled: true });
    expect(actions[0]).toMatchObject({ kind: "web-single", axeEnabled: true });
  });

  it("never runs a code scan in web mode, even when a folder is linked", () => {
    const actions = planScan({ ...BASE, mode: "web" });
    expect(actions.some((a) => a.kind === "code")).toBe(false);
  });

  it("falls back to activeUrl when urls is an empty array", () => {
    const actions = planScan({ ...BASE, mode: "web", urls: [] });
    expect(actions[0]).toMatchObject({ kind: "web-single", url: "https://example.com" });
  });
});

describe("planScan - full mode", () => {
  it("chains web-single then code when project folder is linked", () => {
    const actions = planScan({ ...BASE, mode: "full" });
    expect(actions).toHaveLength(2);
    expect(actions[0].kind).toBe("web-single");
    expect(actions[1]).toEqual({
      kind: "code",
      projectId: 42,
      folder: "/tmp/project",
      url: "https://example.com",
    });
  });

  it("chains web-multi then code when project folder is linked and multiple URLs are provided", () => {
    const actions = planScan({
      ...BASE,
      mode: "full",
      urls: ["https://example.com/a", "https://example.com/b"],
    });
    expect(actions.map((a) => a.kind)).toEqual(["web-multi", "code"]);
  });

  it("runs only the web scan when no project folder is linked", () => {
    const actions = planScan({ ...BASE, mode: "full", projectFolder: null });
    expect(actions).toHaveLength(1);
    expect(actions[0].kind).toBe("web-single");
  });

  it("runs only the web scan when no project is active", () => {
    const actions = planScan({ ...BASE, mode: "full", activeProjectId: null });
    expect(actions).toHaveLength(1);
    expect(actions[0].kind).toBe("web-single");
  });

  it("does NOT error when the folder is missing - Full degrades gracefully to web-only", () => {
    const actions = planScan({ ...BASE, mode: "full", projectFolder: null });
    expect(actions.some((a) => a.kind === "error")).toBe(false);
  });

  it("runs code AFTER web, not before (order matters for sequential await)", () => {
    const actions = planScan({ ...BASE, mode: "full" });
    const webIdx = actions.findIndex((a) => a.kind === "web-single");
    const codeIdx = actions.findIndex((a) => a.kind === "code");
    expect(webIdx).toBeLessThan(codeIdx);
  });
});

describe("planScan - project capabilities", () => {
  const CODE_ONLY = { ...BASE, activeUrl: null };
  const SITE_ONLY = { ...BASE, projectFolder: null };
  const NEITHER = { ...BASE, activeUrl: null, projectFolder: null };

  it("runs only the code half for Full on a project with no site URL", () => {
    const actions = planScan({ ...CODE_ONLY, mode: "full" });
    expect(actions).toEqual([{ kind: "code", projectId: 42, folder: "/tmp/project", url: "" }]);
  });

  it("runs only the web half for Full on a project with no linked folder", () => {
    const actions = planScan({ ...SITE_ONLY, mode: "full" });
    expect(actions.map((a) => a.kind)).toEqual(["web-single"]);
  });

  it("runs both halves for Full when the project has a site and a codebase", () => {
    const actions = planScan({ ...BASE, mode: "full" });
    expect(actions.map((a) => a.kind)).toEqual(["web-single", "code"]);
  });

  it("never emits a web action for a project with no site URL", () => {
    for (const mode of ["full", "web", "code"] as const) {
      const actions = planScan({ ...CODE_ONLY, mode });
      expect(actions.some((a) => a.kind === "web-single" || a.kind === "web-multi")).toBe(false);
    }
  });

  it("errors rather than guessing a URL when Web is asked for without a site", () => {
    const actions = planScan({ ...CODE_ONLY, mode: "web" });
    expect(actions).toHaveLength(1);
    expect(actions[0].kind).toBe("error");
    if (actions[0].kind === "error") {
      expect(actions[0].message).toContain("site URL");
    }
  });

  it("runs the code half in code mode even with no site URL", () => {
    const actions = planScan({ ...CODE_ONLY, mode: "code" });
    expect(actions).toEqual([{ kind: "code", projectId: 42, folder: "/tmp/project", url: "" }]);
  });

  it("errors when the project has neither a site nor a codebase", () => {
    const actions = planScan({ ...NEITHER, mode: "full" });
    expect(actions).toHaveLength(1);
    expect(actions[0].kind).toBe("error");
    if (actions[0].kind === "error") {
      expect(actions[0].message).toContain("Nothing to scan");
    }
  });

  it("ignores blank config URLs rather than scanning an empty string", () => {
    const actions = planScan({ ...CODE_ONLY, mode: "web", urls: ["", "   "] });
    expect(actions[0].kind).toBe("error");
  });
});

describe("planScan - default mode", () => {
  it("treats undefined mode as 'full' at the call site (caller enforces default)", () => {
    const actions = planScan({ ...BASE, mode: "full" });
    expect(actions).toHaveLength(2);
  });
});
