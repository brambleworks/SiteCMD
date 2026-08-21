import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TopBar } from "./TopBar";
import type { EnvironmentRecord, ProjectRecord } from "@/hooks/useProject";

const { startDraggingMock } = vi.hoisted(() => ({
  startDraggingMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    startDragging: startDraggingMock,
  }),
}));

function buildEnv(id: number, url: string): EnvironmentRecord {
  return {
    id,
    url,
    label: url,
    environment: "production",
    source: "manual",
    lastScannedAt: null,
    latestScore: 82,
  };
}

function buildProject(id: number, name: string, framework: string): ProjectRecord {
  return {
    id,
    name,
    path: `/tmp/${name.toLowerCase()}`,
    framework,
    createdAt: "2026-04-19T12:00:00Z",
    environments: [buildEnv(id * 10, `https://${name.toLowerCase()}.test`)],
  };
}

describe("TopBar", () => {
  beforeEach(() => {
    startDraggingMock.mockReset();
  });

  it("keeps the SiteCMD logo out of the window-control row", () => {
    const alpha = buildProject(1, "Alpha", "Astro");

    render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
      />,
    );

    expect(screen.queryByRole("img", { name: "SiteCMD" })).not.toBeInTheDocument();
  });

  it("starts a native window drag from empty top bar space", async () => {
    const alpha = buildProject(1, "Alpha", "Astro");
    const { container } = render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
      />,
    );

    fireEvent.mouseDown(container.querySelector(".app-topbar")!, { button: 0 });

    await waitFor(() => expect(startDraggingMock).toHaveBeenCalledTimes(1));
  });

  it("does not start a window drag from top bar controls", async () => {
    const alpha = buildProject(1, "Alpha", "Astro");
    render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
      />,
    );

    fireEvent.mouseDown(screen.getByRole("button", { name: /alpha/i }), { button: 0 });

    await Promise.resolve();
    expect(startDraggingMock).not.toHaveBeenCalled();
  });

  it("opens project settings from the dropdown without selecting the project", () => {
    const alpha = buildProject(1, "Alpha", "Astro");
    const beta = buildProject(2, "Beta", "Drupal");
    const onSelectProject = vi.fn();
    const onOpenProjectSettings = vi.fn();

    render(
      <TopBar
        projects={[alpha, beta]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={onSelectProject}
        onOpenProjectSettings={onOpenProjectSettings}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /alpha/i }));

    expect(screen.queryByText("Astro")).not.toBeInTheDocument();
    expect(screen.queryByText("Drupal")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Edit Beta project settings" }));

    expect(onOpenProjectSettings).toHaveBeenCalledWith(beta);
    expect(onSelectProject).not.toHaveBeenCalled();
  });

  it("still selects a project from the main row action", () => {
    const alpha = buildProject(1, "Alpha", "Astro");
    const beta = buildProject(2, "Beta", "Drupal");
    const onSelectProject = vi.fn();

    render(
      <TopBar
        projects={[alpha, beta]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={onSelectProject}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /alpha/i }));
    fireEvent.click(screen.getAllByRole("menuitemradio")[1]!);

    expect(onSelectProject).toHaveBeenCalledWith(beta);
  });

  it("does not show raw environment artifact scores in the environment selector", () => {
    const alpha = buildProject(1, "Alpha", "Astro");
    alpha.environments = [
      { ...buildEnv(11, "https://alpha.test"), latestScore: 82 },
      { ...buildEnv(12, "https://staging.alpha.test"), environment: "staging", latestScore: 91 },
    ];

    render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /alpha.test/i }));

    expect(screen.queryByText("82")).not.toBeInTheDocument();
    expect(screen.queryByText("91")).not.toBeInTheDocument();
  });

  it("keeps global settings out of the top bar", () => {
    const alpha = buildProject(1, "Alpha", "Astro");

    render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Settings" })).not.toBeInTheDocument();
  });

  it("renders Run Scan as the top bar primary action", () => {
    const alpha = buildProject(1, "Alpha", "Astro");
    const onRunScan = vi.fn();

    render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
        onRunScan={onRunScan}
      />,
    );

    const runScanButton = screen.getByRole("button", { name: "Run Scan" });
    expect(runScanButton.querySelector("svg")).toHaveAttribute("fill", "currentColor");
    expect(runScanButton.querySelector("svg")).toHaveAttribute("stroke-width", "0");

    fireEvent.click(runScanButton);

    expect(onRunScan).toHaveBeenCalledTimes(1);
  });

  it("disables the top bar scan action while a scan is running", () => {
    const alpha = buildProject(1, "Alpha", "Astro");

    render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
        onRunScan={vi.fn()}
        scanning
      />,
    );

    expect(screen.getByRole("button", { name: "Scanning..." })).toBeDisabled();
  });

  it("renders a configure-scan cog beside Run Scan when onOpenScanConfig is provided", () => {
    const alpha = buildProject(1, "Alpha", "Astro");
    const onOpenScanConfig = vi.fn();

    render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
        onRunScan={vi.fn()}
        onOpenScanConfig={onOpenScanConfig}
      />,
    );

    const cog = screen.getByRole("button", { name: "Configure scan" });
    fireEvent.click(cog);
    expect(onOpenScanConfig).toHaveBeenCalledTimes(1);
  });

  it("disables the configure-scan cog while a scan is running", () => {
    const alpha = buildProject(1, "Alpha", "Astro");

    render(
      <TopBar
        projects={[alpha]}
        activeProject={alpha}
        activeEnv={alpha.environments[0]}
        onSelectProject={vi.fn()}
        onOpenProjectSettings={vi.fn()}
        onSelectEnv={vi.fn()}
        onAddProject={vi.fn()}
        onRunScan={vi.fn()}
        onOpenScanConfig={vi.fn()}
        scanning
      />,
    );

    expect(screen.getByRole("button", { name: "Configure scan" })).toBeDisabled();
  });
});
