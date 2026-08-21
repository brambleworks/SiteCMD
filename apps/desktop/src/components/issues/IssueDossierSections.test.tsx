import { render, screen, within } from "@testing-library/react";
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
