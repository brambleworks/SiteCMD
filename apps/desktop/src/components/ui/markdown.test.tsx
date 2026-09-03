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

  it("renders a javascript: link as inert text, not an anchor", async () => {
    const { container } = render(<Markdown>{"[click](javascript:alert('xss'))"}</Markdown>);
    await waitFor(() => expect(container.querySelector(".markdown-body p")).not.toBeNull());
    // rehype-sanitize drops the href and the renderer drops the anchor with it,
    // so the label survives as plain text with nothing to click.
    expect(container.querySelector("a")).toBeNull();
    expect(container.textContent).toContain("click");
    expect(container.innerHTML.toLowerCase()).not.toContain("javascript:");
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
