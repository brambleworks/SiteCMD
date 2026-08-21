import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { PackageUpdate } from "@/lib/types";
import { SecurityBanner, UpdateSection } from "./UpdateListSections";

function makeUpdate(overrides: Partial<PackageUpdate> = {}): PackageUpdate {
  return {
    name: "lodash",
    currentVersion: "4.17.20",
    latestVersion: "4.17.21",
    ecosystem: "npm",
    updateType: "patch",
    isSecurity: false,
    advisorySeverity: null,
    advisoryUrl: null,
    source: "package.json",
    isDev: true,
    isDeprecated: false,
    deprecationMessage: null,
    currentVersionDeprecated: false,
    isStale: false,
    lastPublished: null,
    workspaceMembers: [],
    ...overrides,
  };
}

function renderSection(updates: PackageUpdate[], onOpenDossier = vi.fn()) {
  render(
    <UpdateSection
      label="PATCH"
      color="text-foreground"
      updates={updates}
      onOpenDossier={onOpenDossier}
    />,
  );
  return onOpenDossier;
}

describe("UpdateSection rows", () => {
  it("opens the dossier from anywhere on the row, not just the package name", () => {
    const onOpenDossier = renderSection([makeUpdate()]);

    fireEvent.click(screen.getByText("4.17.21"));

    expect(onOpenDossier).toHaveBeenCalledWith(expect.objectContaining({ name: "lodash" }));
  });

  it("keeps the row free of per-row verify and copy controls", () => {
    renderSection([makeUpdate()]);

    const row = screen.getByRole("button", { name: /^lodash, / });
    expect(row).toHaveTextContent("lodash");
    expect(screen.queryByRole("button", { name: /verify/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /copy/i })).not.toBeInTheDocument();
  });

  it("drops the dev/dep marker beside the package name", () => {
    renderSection([makeUpdate({ isDev: true })]);

    // Dev vs production is stated in full in the dossier; a bare "dev" next to
    // the name read as a stray token.
    expect(screen.queryByText("dev")).not.toBeInTheDocument();
    expect(screen.queryByText("dep")).not.toBeInTheDocument();
  });

  it("names the row for the package so it does not announce as one run-together string", () => {
    renderSection([makeUpdate()]);

    expect(
      screen.getByRole("button", { name: "lodash, update from 4.17.20 to 4.17.21" }),
    ).toBeInTheDocument();
  });
});

describe("SecurityBanner rows", () => {
  const securityUpdate = makeUpdate({
    name: "next",
    currentVersion: "15.5.14",
    latestVersion: "16.2.10",
    isSecurity: true,
    advisorySeverity: "high",
    advisoryFixedVersion: "16.2.10",
    advisoryUrl: "https://example.com/advisory",
  });

  it("opens the dossier from anywhere on the row", () => {
    const onOpenDossier = vi.fn();
    render(<SecurityBanner updates={[securityUpdate]} onOpenDossier={onOpenDossier} />);

    fireEvent.click(screen.getByText("16.2.10"));

    expect(onOpenDossier).toHaveBeenCalledWith(expect.objectContaining({ name: "next" }));
  });

  it("does not repeat a severity word under every security row", () => {
    render(<SecurityBanner updates={[securityUpdate]} onOpenDossier={vi.fn()} />);

    const row = screen.getByRole("button", { name: /^next, / });
    expect(row).not.toHaveTextContent(/high/i);
    expect(screen.queryByText("HIGH")).not.toBeInTheDocument();
  });
});

describe("SecurityBanner header", () => {
  const securityUpdate = makeUpdate({ name: "next", isSecurity: true, advisorySeverity: "high" });

  it("leaves copy-all to the page header", () => {
    render(<SecurityBanner updates={[securityUpdate]} onOpenDossier={vi.fn()} />);

    // Two copy-everything buttons on one screen invite clicking the wrong one.
    expect(screen.queryByRole("button", { name: /copy fix commands/i })).not.toBeInTheDocument();
    expect(screen.getByText(/SECURITY UPDATES/)).toBeInTheDocument();
  });
});

describe("row title hover", () => {
  it("marks titles and chevrons with the shared Issues list classes", () => {
    const { container } = render(
      <UpdateSection
        label="PATCH"
        color="text-foreground"
        updates={[makeUpdate()]}
        onOpenDossier={vi.fn()}
      />,
    );
    expect(container.querySelector(".list-row__title")).not.toBeNull();
    expect(container.querySelector(".list-row__chevron")).not.toBeNull();
  });

  it("marks security row titles the same way", () => {
    const { container } = render(
      <SecurityBanner
        updates={[makeUpdate({ name: "next", isSecurity: true })]}
        onOpenDossier={vi.fn()}
      />,
    );
    expect(container.querySelector(".list-row__title")).not.toBeNull();
    expect(container.querySelector(".list-row__chevron")).not.toBeNull();
  });
});
