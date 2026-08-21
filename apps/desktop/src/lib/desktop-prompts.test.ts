import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import {
  buildDesktopPromptId,
  buildDesktopPromptTarget,
  buildDesktopWatchPromptCopy,
  clearDesktopPrompts,
  getDesktopPromptById,
  getLatestDesktopPrompt,
  normalizeDesktopPromptReason,
  queueDesktopPrompt,
  resolveDesktopPrompt,
  useDesktopPromptCenter,
  type DesktopPromptEntry,
} from "./desktop-prompts";

function entry(overrides: Partial<DesktopPromptEntry> = {}): DesktopPromptEntry {
  return {
    id: "1:https://example.com:file-edit:src/a.ts",
    projectId: 1,
    url: "https://example.com",
    page: "issues",
    focus: null,
    title: "Re-run checks",
    detail: "src/a.ts was edited",
    relativePath: "src/a.ts",
    absolutePath: "/tmp/app/src/a.ts",
    kind: "file-edit",
    createdAt: 1_000,
    updatedAt: 2_000,
    ...overrides,
  };
}

describe("buildDesktopPromptId", () => {
  it("joins projectId, normalized url, kind, and relativePath", () => {
    expect(buildDesktopPromptId(1, "https://example.com", "file-edit", "src/a.ts")).toBe(
      "1:https://example.com:file-edit:src/a.ts",
    );
  });

  it("normalizes trailing slash so example.com/ and example.com hash the same", () => {
    expect(buildDesktopPromptId(1, "https://example.com/", "k", "src/a.ts")).toBe(
      buildDesktopPromptId(1, "https://example.com", "k", "src/a.ts"),
    );
  });

  it("keeps different projects disjoint", () => {
    const a = buildDesktopPromptId(1, "https://x.com", "k", "f.ts");
    const b = buildDesktopPromptId(2, "https://x.com", "k", "f.ts");
    expect(a).not.toBe(b);
  });
});

describe("buildDesktopPromptTarget", () => {
  it("copies page/projectId/focus/promptId/reason/filePath into an AppTarget", () => {
    const t = buildDesktopPromptTarget(
      entry({ page: "updates", focus: "react", absolutePath: "/tmp/app/src/a.ts" }),
    );
    expect(t.page).toBe("updates");
    expect(t.projectId).toBe(1);
    expect(t.focus).toBe("react");
    expect(t.promptId).toBe("1:https://example.com:file-edit:src/a.ts");
    expect(t.reason).toBe("file-edit");
    expect(t.filePath).toBe("/tmp/app/src/a.ts");
  });

  it("normalizes the url (no trailing slash)", () => {
    const t = buildDesktopPromptTarget(entry({ url: "https://example.com/" }));
    expect(t.url).toBe("https://example.com");
  });

  it("maps missing absolutePath to filePath=null", () => {
    const t = buildDesktopPromptTarget(entry({ absolutePath: null }));
    expect(t.filePath).toBeNull();
  });

  it("normalizes legacy watch kinds into semantic reasons", () => {
    const t = buildDesktopPromptTarget(entry({ page: "search-console", kind: "robots" }));
    expect(t.reason).toBe("changed-search-file");
  });
});

describe("normalizeDesktopPromptReason", () => {
  it("keeps already-normalized reasons intact", () => {
    expect(normalizeDesktopPromptReason("changed-security-file", "issues")).toBe(
      "changed-security-file",
    );
  });

  it("maps page-specific watch kinds to semantic reasons", () => {
    expect(normalizeDesktopPromptReason("dependencies", "updates")).toBe("changed-dependencies");
    expect(normalizeDesktopPromptReason("robots", "search-console")).toBe("changed-search-file");
    expect(normalizeDesktopPromptReason("security-headers", "issues")).toBe(
      "changed-security-file",
    );
    expect(normalizeDesktopPromptReason("auth-session", "issues")).toBe("changed-security-file");
    expect(normalizeDesktopPromptReason("cors-config", "issues")).toBe("changed-security-file");
    expect(normalizeDesktopPromptReason("auth-guard", "issues")).toBe("changed-security-file");
  });
});

describe("buildDesktopWatchPromptCopy", () => {
  it("adds changed-file and search guidance for watched SEO files", () => {
    expect(
      buildDesktopWatchPromptCopy({
        title: "robots.txt changed",
        detail:
          "Crawl directives changed. Verify indexability and search visibility before shipping.",
        page: "search-console",
        reason: "changed-search-file",
        relativePath: "public/robots.txt",
        nextActionLabel: "Verify Search & SEO",
      }),
    ).toEqual({
      title: "robots.txt changed",
      detail:
        "Crawl directives changed. Verify indexability and search visibility before shipping. Changed file: public/robots.txt. This could affect crawl directives, sitemap coverage, or indexability. Recommended next step: Verify Search & SEO.",
    });
  });

  it("surfaces regressed code history in the prompt title and detail", () => {
    expect(
      buildDesktopWatchPromptCopy({
        title: "Header config changed",
        detail:
          "Header or server config changed. Re-check security headers and exposed infrastructure signals.",
        page: "issues",
        reason: "changed-security-file",
        focus: "sec.headers",
        relativePath: "nginx.conf",
        nextActionLabel: "Verify Security",
        memoryCue: {
          label: "Regressed after verified 2h ago",
          tone: "regressed",
          domainLabel: "Security",
        },
      }),
    ).toEqual({
      title: "Header config changed - Security regressed",
      detail:
        "Header or server config changed. Re-check security headers and exposed infrastructure signals. Changed file: nginx.conf. This could affect security headers, hardening, or exposed infrastructure. History: Regressed after verified 2h ago in Security. Recommended next step: Verify Security.",
    });
  });

  it("uses cookie/session-specific impact language for auth-session prompts", () => {
    expect(
      buildDesktopWatchPromptCopy({
        title: "Auth/session config changed",
        detail:
          "Auth, cookie, or session handling changed. Re-check cookie security, CSRF, and session hardening.",
        page: "issues",
        reason: "changed-security-file",
        focus: "sec.cookies",
        relativePath: "src/auth.ts",
        nextActionLabel: "Verify Security",
      }),
    ).toEqual({
      title: "Auth/session config changed",
      detail:
        "Auth, cookie, or session handling changed. Re-check cookie security, CSRF, and session hardening. Changed file: src/auth.ts. This could affect cookie security, CSRF protection, or session handling. Recommended next step: Verify Security.",
    });
  });

  it("uses auth-enforcement-specific impact language for auth guard prompts", () => {
    expect(
      buildDesktopWatchPromptCopy({
        title: "Auth guard changed",
        detail:
          "Route protection or authorization logic changed. Re-check server-side auth enforcement and access control.",
        page: "issues",
        reason: "changed-security-file",
        focus: "sec.auth",
        relativePath: "server/middleware/auth.ts",
        nextActionLabel: "Verify Security",
      }),
    ).toEqual({
      title: "Auth guard changed",
      detail:
        "Route protection or authorization logic changed. Re-check server-side auth enforcement and access control. Changed file: server/middleware/auth.ts. This could affect route protection, authorization, or server-side auth enforcement. Recommended next step: Verify Security.",
    });
  });

  it("uses cors-specific impact language for cors boundary prompts", () => {
    expect(
      buildDesktopWatchPromptCopy({
        title: "CORS or API boundary changed",
        detail:
          "Cross-origin or proxy handling changed. Re-check CORS policy, credential exposure, and API boundary hardening.",
        page: "issues",
        reason: "changed-security-file",
        focus: "sec.cors",
        relativePath: "src/server/cors.ts",
        nextActionLabel: "Verify Security",
      }),
    ).toEqual({
      title: "CORS or API boundary changed",
      detail:
        "Cross-origin or proxy handling changed. Re-check CORS policy, credential exposure, and API boundary hardening. Changed file: src/server/cors.ts. This could affect cross-origin access, API credentials, or proxy configuration. Recommended next step: Verify Security.",
    });
  });

  it("adds dependency-risk guidance for watched lockfile changes", () => {
    expect(
      buildDesktopWatchPromptCopy({
        title: "Dependency files changed",
        detail:
          "Lockfiles or package manifests changed. Re-check dependency risk for this project.",
        page: "updates",
        reason: "changed-dependencies",
        relativePath: "pnpm-lock.yaml",
        nextActionLabel: "Refresh Updates",
      }).detail,
    ).toContain("This could affect dependency versions, advisories, or downstream risk.");
  });
});

describe("getLatestDesktopPrompt", () => {
  const entries = [
    entry({
      id: "a",
      projectId: 1,
      url: "https://x.com",
      page: "issues",
      focus: null,
      updatedAt: 100,
    }),
    entry({
      id: "b",
      projectId: 1,
      url: "https://x.com",
      page: "updates",
      focus: null,
      updatedAt: 200,
    }),
    entry({
      id: "c",
      projectId: 2,
      url: "https://y.com",
      page: "issues",
      focus: null,
      updatedAt: 300,
    }),
    entry({
      id: "d",
      projectId: 1,
      url: "https://x.com",
      page: "issues",
      focus: "csp",
      updatedAt: 400,
    }),
  ];

  it("returns null when nothing matches", () => {
    expect(getLatestDesktopPrompt(entries, { projectId: 999 })).toBeNull();
  });

  it("filters by projectId", () => {
    const result = getLatestDesktopPrompt(entries, { projectId: 2 });
    expect(result?.id).toBe("c");
  });

  it("filters by normalized url", () => {
    const result = getLatestDesktopPrompt(entries, { projectId: 1, url: "https://x.com/" });
    // Array iteration order is preserved, so first match wins
    expect(result?.url).toBe("https://x.com");
  });

  it("filters by page", () => {
    const result = getLatestDesktopPrompt(entries, { projectId: 1, page: "updates" });
    expect(result?.id).toBe("b");
  });

  it("filters by focus", () => {
    const result = getLatestDesktopPrompt(entries, { projectId: 1, focus: "csp" });
    expect(result?.id).toBe("d");
  });
});

describe("queue / resolve / clear lifecycle", () => {
  beforeEach(() => {
    window.localStorage.clear();
    clearDesktopPrompts();
  });

  it("queueDesktopPrompt adds an entry visible via useDesktopPromptCenter", () => {
    const { result } = renderHook(() => useDesktopPromptCenter());
    act(() => {
      queueDesktopPrompt({
        projectId: 1,
        url: "https://x.com",
        page: "issues",
        focus: null,
        title: "Re-verify",
        detail: "edited",
        relativePath: "src/a.ts",
        absolutePath: null,
        kind: "file-edit",
      });
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].projectId).toBe(1);
    expect(result.current[0].url).toBe("https://x.com");
  });

  it("queueing the same id twice replaces and preserves createdAt", () => {
    const { result } = renderHook(() => useDesktopPromptCenter());
    const spy = vi.spyOn(Date, "now").mockReturnValue(1_000);
    act(() => {
      queueDesktopPrompt({
        projectId: 1,
        url: "https://x.com",
        page: "issues",
        focus: null,
        title: "First",
        detail: "a",
        relativePath: "src/a.ts",
        absolutePath: null,
        kind: "k",
      });
    });
    spy.mockReturnValue(5_000);
    act(() => {
      queueDesktopPrompt({
        projectId: 1,
        url: "https://x.com",
        page: "issues",
        focus: null,
        title: "Second",
        detail: "b",
        relativePath: "src/a.ts",
        absolutePath: null,
        kind: "k",
      });
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].title).toBe("Second");
    expect(result.current[0].createdAt).toBe(1_000);
    expect(result.current[0].updatedAt).toBe(5_000);
    spy.mockRestore();
  });

  it("resolveDesktopPrompt removes the entry", () => {
    const { result } = renderHook(() => useDesktopPromptCenter());
    act(() => {
      queueDesktopPrompt({
        projectId: 1,
        url: "https://x.com",
        page: "issues",
        focus: null,
        title: "t",
        detail: "d",
        relativePath: "src/a.ts",
        absolutePath: null,
        kind: "k",
      });
    });
    const id = buildDesktopPromptId(1, "https://x.com", "k", "src/a.ts");
    act(() => {
      resolveDesktopPrompt(id);
    });
    expect(result.current).toHaveLength(0);
  });

  it("getDesktopPromptById still returns a queued entry by its stable id", () => {
    act(() => {
      queueDesktopPrompt({
        projectId: 1,
        url: "https://x.com",
        page: "search-console",
        focus: "seo.robots",
        title: "robots.txt changed",
        detail: "Changed file: public/robots.txt.",
        relativePath: "public/robots.txt",
        absolutePath: "/tmp/app/public/robots.txt",
        kind: "changed-search-file",
      });
    });

    const id = buildDesktopPromptId(1, "https://x.com", "changed-search-file", "public/robots.txt");
    expect(getDesktopPromptById(id)).toMatchObject({
      id,
      page: "search-console",
      focus: "seo.robots",
      relativePath: "public/robots.txt",
    });
  });

  it("clearDesktopPrompts with projectId only removes matching project entries", () => {
    const { result } = renderHook(() => useDesktopPromptCenter());
    act(() => {
      queueDesktopPrompt({
        projectId: 1,
        url: "https://a.com",
        page: "issues",
        focus: null,
        title: "t",
        detail: "",
        relativePath: "x",
        absolutePath: null,
        kind: "k",
      });
      queueDesktopPrompt({
        projectId: 2,
        url: "https://b.com",
        page: "issues",
        focus: null,
        title: "t",
        detail: "",
        relativePath: "y",
        absolutePath: null,
        kind: "k",
      });
    });
    expect(result.current).toHaveLength(2);
    act(() => {
      clearDesktopPrompts({ projectId: 1 });
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].projectId).toBe(2);
  });

  it("clearDesktopPrompts() with no filter empties everything", () => {
    const { result } = renderHook(() => useDesktopPromptCenter());
    act(() => {
      queueDesktopPrompt({
        projectId: 1,
        url: "https://x.com",
        page: "issues",
        focus: null,
        title: "t",
        detail: "",
        relativePath: "x",
        absolutePath: null,
        kind: "k",
      });
    });
    act(() => {
      clearDesktopPrompts();
    });
    expect(result.current).toHaveLength(0);
  });
});

describe("persisted desktop prompt validation", () => {
  it("drops malformed persisted prompts before exposing notification targets", async () => {
    vi.resetModules();
    window.localStorage.clear();
    window.localStorage.setItem(
      "sitecmd_desktop_prompts_v1",
      JSON.stringify([
        entry({ id: "valid", url: "https://example.com/", page: "issues" }),
        entry({ id: "bad-page", page: "settings" as DesktopPromptEntry["page"] }),
        entry({ id: "bad-project", projectId: -1 }),
        entry({ id: "bad-url", url: "https://user:token@example.com" }),
        entry({ id: "bad-time", updatedAt: Number.POSITIVE_INFINITY }),
      ]),
    );

    const prompts = await import("./desktop-prompts");
    const { result } = renderHook(() => prompts.useDesktopPromptCenter());

    expect(result.current).toHaveLength(1);
    expect(result.current[0]).toEqual(
      expect.objectContaining({
        id: "valid",
        projectId: 1,
        page: "issues",
        url: "https://example.com",
      }),
    );
  });
});
