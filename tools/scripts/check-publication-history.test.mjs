import { describe, expect, it } from "vitest";

import {
  candidateHistoryShapeFailures,
  publicationHistoryPathFailures,
} from "./lib/publication-history-rules.mjs";

describe("publicationHistoryPathFailures", () => {
  it("accepts history containing only the public client applications", () => {
    expect(
      publicationHistoryPathFailures([
        "README.md",
        "apps/desktop/src/main.tsx",
        "apps/mcp-server/src/index.ts",
      ]),
    ).toEqual([]);
  });

  it("rejects private strategy records even after deletion", () => {
    expect(
      publicationHistoryPathFailures([
        "docs/engineering/publication-decision.md",
        "docs/engineering/connected-service/commercial-terms-spec.md",
      ]).join("\n"),
    ).toContain("private strategy record");
  });

  it("rejects app trees that do not belong in the public client repository", () => {
    const failures = publicationHistoryPathFailures([
      "apps/sitecmd-telemetry/src/index.ts",
      "apps/sitecmd-telemetry/package.json",
      "apps/sitecmd.com/src/pages/index.astro",
    ]);
    expect(failures).toEqual([
      "public history contains a private/non-client app tree: apps/sitecmd-telemetry/",
      "public history contains a private/non-client app tree: apps/sitecmd.com/",
    ]);
  });
});

describe("candidateHistoryShapeFailures", () => {
  it("accepts one parentless commit", () => {
    expect(candidateHistoryShapeFailures("1", "0123456789abcdef")).toEqual([]);
  });

  it("rejects inherited commits and a parented tip", () => {
    expect(candidateHistoryShapeFailures("42", "tip parent").join("\n")).toContain(
      "exactly one commit",
    );
    expect(candidateHistoryShapeFailures("42", "tip parent").join("\n")).toContain(
      "must have no parent",
    );
  });
});
