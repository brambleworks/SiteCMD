import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import {
  buildPendingVerificationId,
  clearPendingVerification,
  queuePendingVerification,
  queuePendingVerificationMany,
  resolvePendingVerification,
  usePendingVerificationCenter,
} from "./pending-verification";

describe("buildPendingVerificationId", () => {
  it("keeps legacy deep-link ids stable while the queue is retired", () => {
    expect(buildPendingVerificationId(1, "https://example.com/", "accessibility.alt")).toBe(
      "1:https://example.com:accessibility.alt",
    );
    expect(buildPendingVerificationId(1, "https://example.com", "dep.react", "updates")).toBe(
      "1:https://example.com:updates:dep.react",
    );
  });

  it("normalizes equivalent URLs", () => {
    expect(buildPendingVerificationId(1, "https://x.com", "y")).toBe(
      buildPendingVerificationId(1, "https://x.com/", "y"),
    );
    expect(buildPendingVerificationId(1, "https://x.com/deep/", "y")).toBe(
      "1:https://x.com/deep:y",
    );
  });
});

describe("retired pending verification queue", () => {
  beforeEach(() => {
    clearPendingVerification();
    window.localStorage.clear();
  });

  it("does not expose queued entries", () => {
    const { result } = renderHook(() => usePendingVerificationCenter());

    act(() => {
      queuePendingVerification({
        projectId: 1,
        url: "https://example.com",
        itemId: "sec.csp",
        label: "CSP",
        reason: "file edited",
        page: "issues",
      });
      queuePendingVerificationMany([
        {
          projectId: 1,
          url: "https://example.com",
          itemId: "seo.title",
          label: "Title",
          reason: "file edited",
          page: "search-console",
        },
      ]);
    });

    expect(result.current).toEqual([]);
    expect(window.localStorage.getItem("sitecmd_pending_verification_v1")).toBeNull();
  });

  it("ignores legacy persisted entries on startup", () => {
    window.localStorage.setItem(
      "sitecmd_pending_verification_v1",
      JSON.stringify([
        {
          id: "old",
          projectId: 1,
          url: "https://example.com/",
          itemId: "sec.csp",
          label: "CSP",
          reason: "Copied fix prompt",
          page: "issues",
          createdAt: 1,
          updatedAt: 2,
        },
      ]),
    );

    const { result } = renderHook(() => usePendingVerificationCenter());

    expect(result.current).toEqual([]);
  });

  it("resolve and clear remain compatibility no-ops", () => {
    const { result } = renderHook(() => usePendingVerificationCenter());

    act(() => {
      resolvePendingVerification("1:https://example.com:security:sec.csp");
      clearPendingVerification({ projectId: 1 });
    });

    expect(result.current).toEqual([]);
  });
});
