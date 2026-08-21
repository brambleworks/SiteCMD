import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { WatchedFileArrivalBanner } from "./WatchedFileArrivalBanner";

describe("WatchedFileArrivalBanner", () => {
  it("renders prompt context and opens the changed file when requested", () => {
    const onOpenFile = vi.fn();
    const onReview = vi.fn();

    render(
      <WatchedFileArrivalBanner
        prompt={{
          id: "prompt-1",
          projectId: 1,
          url: "https://example.com",
          page: "search-console",
          focus: "seo.robots",
          title: "robots.txt changed",
          detail: "Changed file: public/robots.txt. Recommended next step: Verify Search & SEO.",
          relativePath: "public/robots.txt",
          absolutePath: "/tmp/app/public/robots.txt",
          kind: "changed-search-file",
          createdAt: 1,
          updatedAt: 2,
        }}
        onOpenFile={onOpenFile}
        onReview={onReview}
        reviewLabel="Review matching checks"
      />,
    );

    expect(screen.getByText("robots.txt changed")).toBeInTheDocument();
    expect(screen.getByText("public/robots.txt")).toBeInTheDocument();
    expect(screen.getByText(/Verify Search & SEO/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /open changed file/i }));
    expect(onOpenFile).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /review matching checks/i }));
    expect(onReview).toHaveBeenCalledTimes(1);
  });

  it("dismisses locally", () => {
    render(
      <WatchedFileArrivalBanner
        prompt={{
          id: "prompt-2",
          projectId: 1,
          url: "https://example.com",
          page: "issues",
          focus: null,
          title: "Header config changed",
          detail: "Changed file: nginx.conf.",
          relativePath: "nginx.conf",
          absolutePath: null,
          kind: "changed-security-file",
          createdAt: 1,
          updatedAt: 2,
        }}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /dismiss watched file banner/i }));
    expect(screen.queryByText("Header config changed")).not.toBeInTheDocument();
  });
});
