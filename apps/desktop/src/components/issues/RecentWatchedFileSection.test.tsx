import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { RecentWatchedFileSection } from "./RecentWatchedFileSection";

describe("RecentWatchedFileSection", () => {
  it("renders the watched file title, path, detail, and relative time", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-04-11T19:15:00.000Z"));

    render(
      <RecentWatchedFileSection
        prompt={{
          title: "robots.txt changed",
          detail: "Changed file: public/robots.txt. This could affect crawl directives.",
          relativePath: "public/robots.txt",
          updatedAt: new Date("2026-04-11T19:10:00.000Z").getTime(),
        }}
      />,
    );

    expect(screen.getByText("Recent watched file")).toBeInTheDocument();
    expect(screen.getByText("robots.txt changed")).toBeInTheDocument();
    expect(screen.getByText("public/robots.txt")).toBeInTheDocument();
    expect(screen.getByText(/could affect crawl directives/i)).toBeInTheDocument();
    expect(screen.getByText("5m ago")).toBeInTheDocument();

    vi.useRealTimers();
  });
});
