import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UnifiedFixIssue } from "@/components/issues/IssueList";

const findUnifiedByCheckId = vi.fn();
vi.mock("@/lib/issue-ranking", () => ({
  findUnifiedByCheckId: (...args: unknown[]) => findUnifiedByCheckId(...args),
}));

import { useIssueDossierStack } from "./useIssueDossierStack";

const issue = (checkId: string) => ({ checkId }) as unknown as UnifiedFixIssue;

describe("useIssueDossierStack", () => {
  beforeEach(() => findUnifiedByCheckId.mockReset());

  it("starts empty", () => {
    const { result } = renderHook(() => useIssueDossierStack([], vi.fn()));
    expect(result.current.selectedStack).toEqual([]);
    expect(result.current.selectedIssue).toBeNull();
  });

  it("selectIssue replaces the stack with a single item (not a push)", () => {
    const { result } = renderHook(() => useIssueDossierStack([], vi.fn()));
    const a = issue("a");
    act(() => result.current.selectIssue(a));
    expect(result.current.selectedStack).toEqual([a]);
    expect(result.current.selectedIssue).toBe(a);

    const b = issue("b");
    act(() => result.current.selectIssue(b));
    expect(result.current.selectedStack).toEqual([b]);
  });

  it("openCause pushes the matched cause and goBack pops it", () => {
    const cause = issue("cause");
    findUnifiedByCheckId.mockReturnValue(cause);
    const { result } = renderHook(() => useIssueDossierStack([issue("root")], vi.fn()));

    act(() => result.current.selectIssue(issue("root")));
    act(() => result.current.openCause("cause"));
    expect(result.current.selectedStack).toHaveLength(2);
    expect(result.current.selectedIssue).toBe(cause);

    act(() => result.current.goBack());
    expect(result.current.selectedStack).toHaveLength(1);
    expect(result.current.selectedIssue).toEqual(expect.objectContaining({ checkId: "root" }));
  });

  it("openCause with no match calls onMissingCause and leaves the stack unchanged", () => {
    findUnifiedByCheckId.mockReturnValue(undefined);
    const onMissing = vi.fn();
    const { result } = renderHook(() => useIssueDossierStack([], onMissing));

    act(() => result.current.selectIssue(issue("root")));
    act(() => result.current.openCause("nope"));

    expect(onMissing).toHaveBeenCalledTimes(1);
    expect(result.current.selectedStack).toEqual([expect.objectContaining({ checkId: "root" })]);
  });

  it("caps the cause chain at 5 entries", () => {
    findUnifiedByCheckId.mockImplementation((_issues, id) => issue(id as string));
    const { result } = renderHook(() => useIssueDossierStack([], vi.fn()));

    act(() => result.current.selectIssue(issue("root")));
    for (const id of ["c1", "c2", "c3", "c4", "c5", "c6"]) {
      act(() => result.current.openCause(id));
    }

    expect(result.current.selectedStack).toHaveLength(5);
    expect(result.current.selectedIssue).toEqual(expect.objectContaining({ checkId: "c6" }));
  });

  it("goBack on a single-item stack is a no-op", () => {
    const { result } = renderHook(() => useIssueDossierStack([], vi.fn()));
    act(() => result.current.selectIssue(issue("only")));
    act(() => result.current.goBack());
    expect(result.current.selectedStack).toHaveLength(1);
  });

  it("closeIssue and resetIssueStack both empty the stack", () => {
    const { result } = renderHook(() => useIssueDossierStack([], vi.fn()));

    act(() => result.current.selectIssue(issue("a")));
    act(() => result.current.closeIssue());
    expect(result.current.selectedStack).toEqual([]);

    act(() => result.current.selectIssue(issue("b")));
    act(() => result.current.resetIssueStack());
    expect(result.current.selectedStack).toEqual([]);
  });
});
