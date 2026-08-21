import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import {
  clearIssuesBadge,
  clearIssuesBadgeForProject,
  setIssuesBadgeFromSummary,
  useIssuesBadge,
} from "./issues-badge";

describe("issues badge", () => {
  afterEach(() => {
    clearIssuesBadge();
  });

  it("counts code issues from loaded detail when available", () => {
    const { result } = renderHook(() => useIssuesBadge(7));

    act(() => {
      setIssuesBadgeFromSummary(7, {
        totalCount: 4,
        criticalCount: 2,
      });
    });

    expect(result.current).toEqual({
      projectId: 7,
      total: 4,
      critical: 2,
    });
  });

  it("falls back to code scan summary counts when detail is not loaded yet", () => {
    const { result } = renderHook(() => useIssuesBadge(7));

    act(() => {
      setIssuesBadgeFromSummary(7, {
        totalCount: 7,
        criticalCount: 3,
      });
    });

    expect(result.current).toEqual({
      projectId: 7,
      total: 7,
      critical: 3,
    });
  });

  it("retains last-known issue badges per project", () => {
    const { result: alpha } = renderHook(() => useIssuesBadge(7));
    const { result: beta } = renderHook(() => useIssuesBadge(8));

    act(() => {
      setIssuesBadgeFromSummary(7, {
        totalCount: 4,
        criticalCount: 2,
      });
      setIssuesBadgeFromSummary(8, {
        totalCount: 9,
        criticalCount: 1,
      });
    });

    expect(alpha.current).toEqual({
      projectId: 7,
      total: 4,
      critical: 2,
    });
    expect(beta.current).toEqual({
      projectId: 8,
      total: 9,
      critical: 1,
    });
  });

  it("clears only the matching project badge", () => {
    const { result: alpha } = renderHook(() => useIssuesBadge(7));
    const { result: beta } = renderHook(() => useIssuesBadge(8));

    act(() => {
      setIssuesBadgeFromSummary(7, {
        totalCount: 4,
        criticalCount: 2,
      });
      setIssuesBadgeFromSummary(8, {
        totalCount: 9,
        criticalCount: 1,
      });
      clearIssuesBadgeForProject(7);
    });

    expect(alpha.current).toBeNull();
    expect(beta.current).toEqual({
      projectId: 8,
      total: 9,
      critical: 1,
    });
  });
});
