import { describe, expect, it } from "vitest";

import { createAppQueryClient } from "./query-client";

describe("app query cache defaults", () => {
  it("trusts event invalidation and does not refetch on ordinary page switches or focus", () => {
    const defaults = createAppQueryClient().getDefaultOptions().queries;

    expect(defaults?.staleTime).toBe(Infinity);
    expect(defaults?.refetchOnWindowFocus).toBe(false);
    expect(defaults?.refetchOnReconnect).toBe(false);
    expect(defaults?.retry).toBe(false);
  });
});
