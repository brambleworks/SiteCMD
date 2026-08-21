import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CheckResult, FixLocation } from "@/lib/types";

const {
  invokeMock,
  toastSuccessMock,
  toastErrorMock,
  queuePendingVerificationMock,
  runProjectCommandMock,
  openPathInEditorMock,
  revealPathMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
  queuePendingVerificationMock: vi.fn(),
  runProjectCommandMock: vi.fn(),
  openPathInEditorMock: vi.fn(() => Promise.resolve()),
  revealPathMock: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    toast: vi.fn(),
    success: toastSuccessMock,
    error: toastErrorMock,
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));
vi.mock("@/lib/desktop-actions", () => ({
  openPathInEditor: openPathInEditorMock,
  revealPath: revealPathMock,
  runProjectCommand: runProjectCommandMock,
  isProjectCommandCancelled: vi.fn(() => false),
}));
vi.mock("@/lib/pending-verification", () => ({
  queuePendingVerification: queuePendingVerificationMock,
}));
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { getIssuePageTarget, useIssueDossierActions } from "./useIssueDossierActions";

const ISSUE: CheckResult = {
  checkId: "security.csp",
  category: "security",
  title: "Content Security Policy is missing",
  description: "Responses do not include a CSP header.",
  status: "fail",
  severity: "high",
  fixPrompt: null,
  manualFix: null,
  rawData: null,
  confidence: "high",
};

const NORMALIZED_URL = normalizeAppUrlForKey("https://example.com");

const FIX_LOCATION: FixLocation = {
  label: "Middleware",
  reason: "Sets response headers",
  relativePath: "src/middleware.ts",
  absolutePath: "/tmp/project/src/middleware.ts",
};

function renderActions(overrides: Partial<Parameters<typeof useIssueDossierActions>[0]> = {}) {
  return renderHook(() =>
    useIssueDossierActions({
      issue: ISSUE,
      projectId: 7,
      url: "https://example.com",
      projectPath: "/tmp/project",
      page: "issues",
      focus: "security",
      reasons: {
        openedPath: "Opened likely fix file",
        revealedPath: "Revealed likely fix file",
        ranCommand: "Ran fix command",
      },
      ...overrides,
    }),
  );
}

describe("useIssueDossierActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockResolvedValue([]);
  });

  describe("fix-location resolution", () => {
    it("resolves fix locations for every install with a project path", async () => {
      invokeMock.mockResolvedValue([FIX_LOCATION]);
      const { result } = renderActions();

      await waitFor(() => expect(result.current.correlatedFiles).toEqual([FIX_LOCATION]));

      expect(invokeMock).toHaveBeenCalledWith("resolve_fix_locations_for_check", {
        checkId: "security.csp",
        projectId: 7,
      });
      expect(result.current.primaryCorrelatedFile).toEqual(FIX_LOCATION);
    });

    it("does not resolve without a project path", async () => {
      const { result } = renderActions({ projectPath: null });

      await act(async () => {});

      expect(invokeMock).not.toHaveBeenCalled();
      expect(result.current.correlatedFiles).toEqual([]);
      expect(result.current.primaryCorrelatedFile).toBeNull();
    });
  });

  describe("queueWorkingState", () => {
    it("no-ops without a projectId", async () => {
      const { result } = renderActions({ projectId: undefined });

      await act(async () => {
        await result.current.queueWorkingState("Did a thing");
      });

      expect(queuePendingVerificationMock).not.toHaveBeenCalled();
    });

    it("queues pending verification with a projectId", async () => {
      const { result } = renderActions();

      await act(async () => {
        await result.current.queueWorkingState("Did a thing", "/tmp/project/src/app.ts");
      });

      expect(queuePendingVerificationMock).toHaveBeenCalledTimes(1);
      expect(queuePendingVerificationMock).toHaveBeenCalledWith({
        projectId: 7,
        url: NORMALIZED_URL,
        itemId: "security.csp",
        label: ISSUE.title,
        reason: "Did a thing",
        page: "issues",
        focus: "security",
        filePath: "/tmp/project/src/app.ts",
      });
    });

    it("falls back to the project path when no file path is given", async () => {
      const { result } = renderActions();

      await act(async () => {
        await result.current.queueWorkingState("Did a thing");
      });

      expect(queuePendingVerificationMock).toHaveBeenCalledWith(
        expect.objectContaining({ filePath: "/tmp/project" }),
      );
    });
  });

  describe("runFirstCommand", () => {
    it("does not queue verification when the command fails", async () => {
      runProjectCommandMock.mockResolvedValue({
        exitCode: 1,
        stdout: "",
        stderr: "boom",
        success: false,
      });
      const { result } = renderActions();

      await act(async () => {
        await result.current.runFirstCommand(["npm run fix"]);
      });

      expect(runProjectCommandMock).toHaveBeenCalledWith("/tmp/project", "npm run fix");
      expect(queuePendingVerificationMock).not.toHaveBeenCalled();
      expect(toastErrorMock).toHaveBeenCalledWith("Command failed", "boom");
      expect(result.current.lastCommandResult?.success).toBe(false);
    });

    it("queues verification when the command succeeds", async () => {
      runProjectCommandMock.mockResolvedValue({
        exitCode: 0,
        stdout: "done",
        stderr: "",
        success: true,
      });
      const { result } = renderActions();

      await act(async () => {
        await result.current.runFirstCommand(["npm run fix"]);
      });

      expect(queuePendingVerificationMock).toHaveBeenCalledTimes(1);
      expect(queuePendingVerificationMock).toHaveBeenCalledWith(
        expect.objectContaining({ reason: "Ran fix command" }),
      );
      expect(toastSuccessMock).toHaveBeenCalledWith("Command finished", "done");
      expect(result.current.lastCommandResult?.success).toBe(true);
    });
  });

  describe("open and reveal handlers", () => {
    it("records working state when opening a file", async () => {
      const { result } = renderActions();

      await act(async () => {
        await result.current.openFile(FIX_LOCATION);
      });

      expect(openPathInEditorMock).toHaveBeenCalledWith(FIX_LOCATION.absolutePath);
      expect(queuePendingVerificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          reason: "Opened likely fix file",
          filePath: FIX_LOCATION.absolutePath,
        }),
      );
      expect(toastSuccessMock).toHaveBeenCalledWith("Opened in editor", FIX_LOCATION.relativePath);
    });

    it("records working state when revealing a file", async () => {
      const { result } = renderActions();

      await act(async () => {
        await result.current.revealFile(FIX_LOCATION);
      });

      expect(revealPathMock).toHaveBeenCalledWith(FIX_LOCATION.absolutePath);
      expect(queuePendingVerificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          reason: "Revealed likely fix file",
          filePath: FIX_LOCATION.absolutePath,
        }),
      );
    });

    it("prefers the dossier's preferred location when opening the editor", async () => {
      const { result } = renderActions({
        preferredLocation: {
          absolutePath: "/tmp/project/src/pages/index.astro",
          relativePath: "src/pages/index.astro",
        },
      });

      await act(async () => {
        await result.current.openEditor();
      });

      expect(openPathInEditorMock).toHaveBeenCalledWith("/tmp/project/src/pages/index.astro");
      expect(toastSuccessMock).toHaveBeenCalledWith("Opened in editor", "src/pages/index.astro");
    });
  });
});

describe("getIssuePageTarget", () => {
  it("routes seo issues to the search-console page", () => {
    expect(getIssuePageTarget({ ...ISSUE, category: "seo" })).toEqual({
      page: "search-console",
      focus: null,
    });
  });

  it("routes everything else to the issues page filtered by category", () => {
    expect(getIssuePageTarget(ISSUE)).toEqual({ page: "issues", focus: "security" });
  });
});
