import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const openUrl = vi.fn();
vi.mock("@/lib/open-url", () => ({
  openUrl: (url: string) => openUrl(url),
}));

import MarkdownRenderer from "./markdown-renderer";

// Fix prompts and issue evidence embed scanned-site and repository text. A
// page <title> such as `[prize](https://attacker.example/x)` must never become
// a raw anchor that navigates the app window, and `![x](https://...)` must
// never become an <img> that beacons to an arbitrary origin on expand.
describe("MarkdownRenderer untrusted link and image handling", () => {
  it("renders links through the confirmed external opener, never a raw anchor", () => {
    const { container } = render(
      <MarkdownRenderer>{"Claim [your prize](https://attacker.example/x) now"}</MarkdownRenderer>,
    );

    expect(container.querySelector("a")).toBeNull();
    const link = screen.getByRole("link", { name: "your prize" });
    fireEvent.click(link);
    expect(openUrl).toHaveBeenCalledWith("https://attacker.example/x");
  });

  it("drops images so scanned content cannot beacon out on render", () => {
    const { container } = render(
      <MarkdownRenderer>
        {"Buy now ![beacon](https://attacker.example/p.png) today"}
      </MarkdownRenderer>,
    );

    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("Buy now");
  });
});
