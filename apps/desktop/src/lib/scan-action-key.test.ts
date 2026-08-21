import { describe, expect, it } from "vitest";

import { createScanActionKey } from "./scan-action-key";

describe("createScanActionKey", () => {
  it("creates a fresh namespaced idempotency key for every deliberate action", () => {
    const first = createScanActionKey("manual-full");
    const second = createScanActionKey("manual-full");

    expect(first).toMatch(/^manual-full:/);
    expect(second).toMatch(/^manual-full:/);
    expect(second).not.toBe(first);
  });
});
