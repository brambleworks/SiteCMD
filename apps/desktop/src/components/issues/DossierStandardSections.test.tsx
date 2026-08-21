import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DossierVerifyCallout } from "./DossierStandardSections";

const cardsSecondaryCss = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "../../styles/cards-secondary.css"),
  "utf8",
);

describe("DossierVerifyCallout", () => {
  it("uses the same card treatment as overview nested info blocks", () => {
    render(<DossierVerifyCallout>Run a fresh scan.</DossierVerifyCallout>);

    expect(screen.getByText("How to check it").closest("p")).toHaveClass("details-section-label");
    expect(screen.getByText("Run a fresh scan.")).toHaveClass("text-body-muted", "text-relaxed");
    expect(screen.getByText("Run a fresh scan.").parentElement).toHaveClass("nested-info-card");
    expect(cardsSecondaryCss).toMatch(/\.nested-info-card > \* \+ \* \{[^}]*margin-top/);
  });
});
