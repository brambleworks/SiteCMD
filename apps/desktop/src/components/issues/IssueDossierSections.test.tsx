import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { IssueProofSection, IssueWhereLivesSection } from "./IssueDossierSections";

describe("Issue dossier sections", () => {
  it("renders file locations without redundant item labels", () => {
    render(
      <IssueWhereLivesSection
        pages={[]}
        files={[
          {
            key: "primary",
            label: "Affected file",
            relativePath: "src/app/page.tsx",
            reason: "Owns the page markup.",
          },
          {
            key: "secondary",
            label: "Same pattern",
            relativePath: "src/app/pricing/page.tsx",
            reason: "Repeats the same markup pattern.",
          },
        ]}
      />,
    );

    expect(screen.getByText("src/app/page.tsx")).toBeInTheDocument();
    expect(screen.getByText("Owns the page markup.")).toBeInTheDocument();
    expect(screen.queryByText("Affected file")).not.toBeInTheDocument();
    expect(screen.queryByText("Same pattern")).not.toBeInTheDocument();
  });

  it("bounds the affected-file list and reveals the rest through the pager", () => {
    const files = Array.from({ length: 3000 }, (_, index) => ({
      key: `file-${index + 1}`,
      label: "Affected file",
      relativePath: `src/pages/page-${index + 1}.tsx`,
    }));

    const { container } = render(<IssueWhereLivesSection pages={[]} files={files} />);

    // An issue with thousands of locations mounts one page of rows.
    expect(container.querySelectorAll(".dossier-where-row")).toHaveLength(20);
    expect(screen.getByText("src/pages/page-1.tsx")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next location page" }));

    expect(container.querySelectorAll(".dossier-where-row")).toHaveLength(20);
    expect(screen.getByText("src/pages/page-21.tsx")).toBeInTheDocument();
    expect(screen.queryByText("src/pages/page-1.tsx")).not.toBeInTheDocument();
  });

  it("bounds the affected-page list and reveals the rest through the pager", () => {
    const pages = Array.from({ length: 3000 }, (_, index) => ({
      key: `page-${index + 1}`,
      label: `https://example.com/page-${index + 1}`,
    }));

    const { container } = render(<IssueWhereLivesSection pages={pages} files={[]} />);

    expect(container.querySelectorAll(".dossier-where-row")).toHaveLength(20);
    expect(screen.getByText("https://example.com/page-1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next location page" }));

    expect(container.querySelectorAll(".dossier-where-row")).toHaveLength(20);
    expect(screen.getByText("https://example.com/page-21")).toBeInTheDocument();
    expect(screen.queryByText("https://example.com/page-1")).not.toBeInTheDocument();
  });

  it("reopens the location list on its first page for a different issue", () => {
    const filesFor = (prefix: string) =>
      Array.from({ length: 60 }, (_, index) => ({
        key: `${prefix}-${index + 1}`,
        label: "Affected file",
        relativePath: `src/${prefix}/page-${index + 1}.tsx`,
      }));

    const { rerender } = render(<IssueWhereLivesSection pages={[]} files={filesFor("first")} />);

    fireEvent.click(screen.getByRole("button", { name: "Next location page" }));
    expect(screen.getByText("src/first/page-21.tsx")).toBeInTheDocument();

    rerender(<IssueWhereLivesSection pages={[]} files={filesFor("second")} />);

    expect(screen.getByText("src/second/page-1.tsx")).toBeInTheDocument();
    expect(screen.queryByText("src/second/page-21.tsx")).not.toBeInTheDocument();
  });

  it("pages the affected files and affected pages independently", () => {
    const pages = Array.from({ length: 60 }, (_, index) => ({
      key: `page-${index + 1}`,
      label: `https://example.com/page-${index + 1}`,
    }));
    const files = Array.from({ length: 60 }, (_, index) => ({
      key: `file-${index + 1}`,
      label: "Affected file",
      relativePath: `src/pages/page-${index + 1}.tsx`,
    }));

    const { container } = render(<IssueWhereLivesSection pages={pages} files={files} />);

    expect(container.querySelectorAll(".dossier-where-row")).toHaveLength(40);

    const [pagesPager, filesPager] = screen.getAllByRole("button", {
      name: "Next location page",
    });
    fireEvent.click(filesPager);

    expect(screen.getByText("https://example.com/page-1")).toBeInTheDocument();
    expect(screen.getByText("src/pages/page-21.tsx")).toBeInTheDocument();
    expect(pagesPager).toBeInTheDocument();
  });

  it("shows proof content immediately like the other numbered sections", () => {
    render(
      <IssueProofSection summary="Captured scan evidence.">
        <div>Observed response header was missing.</div>
      </IssueProofSection>,
    );

    const proofSection = screen.getByText("Evidence").closest("section");
    expect(proofSection).not.toBeNull();
    expect(within(proofSection as HTMLElement).getByText("Captured scan evidence.")).toBeVisible();
    expect(
      within(proofSection as HTMLElement).getByText("Observed response header was missing."),
    ).toBeVisible();
    expect(within(proofSection as HTMLElement).queryByRole("button")).not.toBeInTheDocument();
  });
});
