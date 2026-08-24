import { describe, expect, it } from "vitest";
import {
  fencedLanguages,
  loadHighlightLanguages,
  loadedHighlightLanguages,
} from "./markdown-languages";

describe("markdown grammars load on demand", () => {
  it("finds the canonical grammar for every fenced block, resolving aliases", () => {
    const source =
      "```ts\nconst a = 1;\n```\n\n```html\n<b/>\n```\n\n```brainfuck\n+\n```\n\n```TS\n```";
    expect(fencedLanguages(source)).toEqual(["typescript", "xml"]);
  });

  it("loads only the grammars a document needs and keeps them for later", async () => {
    expect(loadedHighlightLanguages()).not.toHaveProperty("rust");
    const languages = await loadHighlightLanguages(["rust"]);
    expect(typeof languages.rust).toBe("function");
    expect(languages).not.toHaveProperty("python");
    expect(loadedHighlightLanguages()).toHaveProperty("rust");
  });
});
