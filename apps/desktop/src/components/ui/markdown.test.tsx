import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Markdown } from "./markdown";

describe("Markdown sanitization", () => {
  it("never renders raw <script> elements from markdown source", () => {
    const { container } = render(
      <Markdown>{"hello <script>window.evil=1</script> world"}</Markdown>,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.innerHTML).not.toContain("<script");
  });

  it("strips javascript: hrefs from markdown links", () => {
    const { container } = render(<Markdown>{"[click](javascript:alert('xss'))"}</Markdown>);
    const link = container.querySelector("a");
    const href = link?.getAttribute("href") ?? "";
    // Whether the sanitizer drops the href entirely or rewrites the
    // scheme, the resulting attribute must not point at a javascript URL.
    expect(href.toLowerCase()).not.toContain("javascript:");
  });

  it("preserves rehype-highlight class names on code blocks", () => {
    const { container } = render(<Markdown>{"```ts\nconst x = 1;\n```"}</Markdown>);
    const code = container.querySelector("code");
    expect(code?.className).toMatch(/language-ts/);
  });
});
