/// <reference types="node" />

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const SOURCE = readFileSync(
  path.join(path.dirname(fileURLToPath(import.meta.url)), "EventsPage.tsx"),
  "utf8",
);

describe("Activity toolbar icon buttons", () => {
  it("give every icon-only button an accessible name", () => {
    // `[^>]*` alone would stop at the `>` inside an `onClick={() => ...}` arrow
    // function, so `=>` is matched as an atomic unit before falling back to
    // "any character but a bare tag close".
    const iconButtons =
      SOURCE.match(/<Button(?:=>|[^>])*className="icon-btn-sm"(?:=>|[^>])*>/gs) ?? [];
    expect(iconButtons.length).toBeGreaterThanOrEqual(5);
    const unnamed = iconButtons.filter((tag) => !/aria-label=/.test(tag));
    expect(unnamed).toEqual([]);
  });
});
