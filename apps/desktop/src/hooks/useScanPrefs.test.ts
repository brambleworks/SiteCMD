import { describe, expect, it } from "vitest";

import { parseScanPreferences } from "./useScanPrefs";

describe("parseScanPreferences", () => {
  it("clamps malformed persisted timeout and retention values to supported bounds", () => {
    expect(
      parseScanPreferences({
        timeout: -5,
        retentionLimit: 10_000,
        categories: {
          security: false,
          performance: true,
          seo: "yes",
        },
      }),
    ).toEqual({
      timeout: 10,
      retentionLimit: 100,
      categories: {
        security: true,
        performance: true,
        seo: true,
        accessibility: true,
        compliance: true,
        config: true,
      },
    });
  });

  it("uses defaults for non-finite persisted numbers", () => {
    expect(
      parseScanPreferences({
        timeout: Number.NaN,
        retentionLimit: Infinity,
        categories: null,
      }),
    ).toMatchObject({
      timeout: 30,
      retentionLimit: 50,
    });
  });
});
