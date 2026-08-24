import { render, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Markdown } from "./markdown";

describe("Markdown sanitization", () => {
  it("never renders raw <script> elements from markdown source", async () => {
    const { container } = render(
      <Markdown>{"hello <script>window.evil=1</script> world"}</Markdown>,
    );
    await waitFor(() => expect(container.querySelector(".markdown-body p")).not.toBeNull());
    expect(container.querySelector("script")).toBeNull();
    expect(container.innerHTML).not.toContain("<script");
  });

  it("strips javascript: hrefs from markdown links", async () => {
    const { container } = render(<Markdown>{"[click](javascript:alert('xss'))"}</Markdown>);
    await waitFor(() => expect(container.querySelector("a")).not.toBeNull());
    const href = container.querySelector("a")?.getAttribute("href") ?? "";
    expect(href.toLowerCase()).not.toContain("javascript:");
  });

  it("shows the raw text while the renderer chunk loads", () => {
    const { container } = render(<Markdown>{"plain"}</Markdown>);
    expect(container.textContent).toContain("plain");
  });

  it("highlights fenced code once its grammar has loaded", async () => {
    const { container } = render(<Markdown>{"```ts\nconst x = 1;\n```"}</Markdown>);
    await waitFor(() => expect(container.querySelector("code")?.className).toMatch(/language-ts/));
    await waitFor(() => expect(container.querySelector(".hljs-keyword")).not.toBeNull());
  });
});
