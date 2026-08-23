/// <reference types="node" />

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const STYLES = path.dirname(fileURLToPath(import.meta.url));

function cssFiles(dir: string, files: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) cssFiles(full, files);
    else if (entry.name.endsWith(".css")) files.push(full);
  }
  return files;
}

describe("reduced motion", () => {
  const animations = readFileSync(path.join(STYLES, "animations.css"), "utf8");

  it("disables every animation and transition when the OS asks for reduced motion", () => {
    const block = animations.match(/@media \(prefers-reduced-motion: reduce\) \{([\s\S]*?)\n\}/);
    expect(block, "animations.css needs one global prefers-reduced-motion block").not.toBeNull();
    const body = block![1];
    expect(body).toMatch(/^\s*\*,\s*\n\s*\*::before,\s*\n\s*\*::after \{/m);
    expect(body).toContain("animation-duration: 0.01ms !important;");
    expect(body).toContain("animation-iteration-count: 1 !important;");
    expect(body).toContain("transition-duration: 0.01ms !important;");
    expect(body).toContain("scroll-behavior: auto !important;");
  });

  it("keeps every keyframe in animations.css so the global block covers it", () => {
    const strays = cssFiles(STYLES)
      .filter((file) => !file.endsWith("animations.css"))
      .filter((file) => /@keyframes\s/.test(readFileSync(file, "utf8")))
      .map((file) => path.relative(STYLES, file));
    expect(strays).toEqual([]);
  });

  it("has exactly one reduced-motion block, not per-component copies", () => {
    const copies = cssFiles(STYLES)
      .filter((file) => !file.endsWith("animations.css"))
      .filter((file) => readFileSync(file, "utf8").includes("prefers-reduced-motion"))
      .map((file) => path.relative(STYLES, file));
    expect(copies).toEqual([]);
  });
});
