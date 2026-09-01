import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, openMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: openMock,
}));

import { AddProjectForm } from "./AddProjectForm";
import {
  getProjectUrlIdentityKey,
  inferProjectEnvironmentFromUrl,
  resolveProjectEnvironmentForUrl,
} from "@/lib/project-environments";

describe("inferProjectEnvironmentFromUrl", () => {
  it("maps obvious local, development, staging, and production URLs", () => {
    expect(inferProjectEnvironmentFromUrl("localhost:4321")).toBe("local");
    expect(inferProjectEnvironmentFromUrl("https://dev.example.com")).toBe("development");
    expect(inferProjectEnvironmentFromUrl("https://preview-my-app.vercel.app")).toBe("staging");
    expect(inferProjectEnvironmentFromUrl("https://example.com")).toBe("production");
  });

  it("treats localhost and 127 loopback aliases as the same project URL", () => {
    expect(getProjectUrlIdentityKey("http://localhost:4321/")).toBe(
      getProjectUrlIdentityKey("http://127.0.0.1:4321"),
    );
    expect(resolveProjectEnvironmentForUrl("http://127.0.0.1:4321", "production")).toBe("local");
  });
});

describe("AddProjectForm primary environment", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
    openMock.mockResolvedValue(null);
    invokeMock.mockResolvedValue(null);
  });

  it("auto-infers the primary environment from the first URL until the user overrides it", () => {
    render(<AddProjectForm onCreated={vi.fn()} onCancel={vi.fn()} />);

    const environmentSelect = screen.getByDisplayValue("Production") as HTMLSelectElement;
    const primaryUrlInput = screen.getByLabelText("Site URL") as HTMLInputElement;

    fireEvent.change(primaryUrlInput, { target: { value: "localhost:4321" } });
    expect(environmentSelect.value).toBe("local");

    fireEvent.change(environmentSelect, { target: { value: "production" } });
    fireEvent.change(primaryUrlInput, { target: { value: "localhost:4444" } });
    expect(environmentSelect.value).toBe("production");
  });

  it("offers a site URL and a folder as two ways to satisfy one requirement", () => {
    const { container } = render(<AddProjectForm onCreated={vi.fn()} onCancel={vi.fn()} />);

    expect(screen.getByLabelText("Site URL")).toBeInTheDocument();
    expect(screen.getByText("Source Code")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /select folder/i })).toBeInTheDocument();

    expect(container.querySelectorAll(".requirement-list")).toHaveLength(3);

    expect(screen.getByText(/Add a site URL, a folder, or both/)).toBeInTheDocument();
    expect(screen.queryByText(/Optional if/)).not.toBeInTheDocument();
  });

  it("unlocks creation as soon as either half is provided", () => {
    render(<AddProjectForm onCreated={vi.fn()} onCancel={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "Mine" } });
    expect(screen.getByRole("button", { name: /create project/i })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Site URL"), { target: { value: "mysite.com" } });
    expect(screen.getByRole("button", { name: /create project/i })).toBeEnabled();
  });

  it("always renders the form with no project-limit gate", () => {
    const onCancel = vi.fn();

    render(<AddProjectForm onCreated={vi.fn()} onCancel={onCancel} />);

    expect(screen.getByLabelText("Site URL")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /select folder/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /create project/i })).toBeInTheDocument();
    expect(screen.queryByText("Project limit reached")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /upgrade plan/i })).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("get_usage_stats");

    fireEvent.click(screen.getByRole("button", { name: /^cancel$/i }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("adds environment rows inline, with no Advanced disclosure", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "add_project_by_url") return 21;
      return null;
    });

    render(<AddProjectForm onCreated={vi.fn()} onCancel={vi.fn()} />);

    expect(screen.queryByText(/Advanced options/)).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "Multi" } });
    fireEvent.change(screen.getByLabelText("Site URL"), { target: { value: "mysite.com" } });

    fireEvent.click(screen.getByRole("button", { name: /add environment/i }));
    fireEvent.change(screen.getByLabelText("Environment 2 URL"), {
      target: { value: "staging.mysite.com" },
    });

    fireEvent.click(screen.getByRole("button", { name: /create project/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "add_project_by_url",
        expect.objectContaining({ url: "https://mysite.com" }),
      );
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "add_environment_url",
        expect.objectContaining({ url: "https://staging.mysite.com", environment: "staging" }),
      );
    });
  });

  it("creates a code-only project instead of inventing a localhost URL", async () => {
    openMock.mockResolvedValue("/tmp/sitecmd-codeonly");
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_project_urls") {
        return {
          path: "/tmp/sitecmd-codeonly",
          name: "sitecmd-codeonly",
          framework: "Vite",
          urls: [],
        };
      }
      if (command === "add_project") return 7;
      return null;
    });
    const onCreated = vi.fn();

    render(<AddProjectForm onCreated={onCreated} onCancel={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "Code Only" } });
    fireEvent.click(screen.getByRole("button", { name: /select folder/i }));

    await waitFor(() => {
      expect(screen.getByText("/tmp/sitecmd-codeonly")).toBeInTheDocument();
    });

    // The folder alone is a complete project, and the notice says so rather
    // than announcing a URL the user never asked for.
    expect(screen.getByText(/No environment URL was detected/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /create project/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("add_project", expect.objectContaining({ urls: [] }));
    });

    const addProjectCalls = invokeMock.mock.calls.filter(([command]) => command === "add_project");
    expect(JSON.stringify(addProjectCalls)).not.toContain("localhost");
    expect(onCreated).toHaveBeenCalledWith(7);
  });

  it("does not prefill a detected dev-server URL, but still prefills a detected live site", async () => {
    openMock.mockResolvedValue("/tmp/sitecmd-mixed");
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_project_urls") {
        return {
          path: "/tmp/sitecmd-mixed",
          name: "sitecmd-mixed",
          framework: "Astro",
          urls: [
            // A real read of a real file, but still only a dev server: it is
            // listening only while the user happens to be running it.
            { url: "http://localhost:4321", environment: "local", source: "package.json" },
            { url: "https://example.com", environment: "production", source: ".env" },
          ],
        };
      }
      if (command === "add_project") return 11;
      return null;
    });
    const onCreated = vi.fn();

    render(<AddProjectForm onCreated={onCreated} onCancel={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /select folder/i }));

    await waitFor(() => {
      expect(screen.getByText("/tmp/sitecmd-mixed")).toBeInTheDocument();
    });

    expect((screen.getByLabelText("Site URL") as HTMLInputElement).value).toBe(
      "https://example.com",
    );

    fireEvent.click(screen.getByRole("button", { name: /create project/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "add_project",
        expect.objectContaining({
          urls: [{ url: "https://example.com", environment: "production", source: "manual" }],
        }),
      );
    });
    const addProjectCalls = invokeMock.mock.calls.filter(([command]) => command === "add_project");
    expect(JSON.stringify(addProjectCalls)).not.toContain("localhost");
  });

  it("keeps a detected DDEV URL on Local instead of adding it as another production site", async () => {
    openMock.mockResolvedValue("/tmp/smarthomeu");
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_project_urls") {
        return {
          path: "/tmp/smarthomeu",
          name: "smarthomeu",
          framework: "Drupal",
          // DDEV publishes this hostname for a site served from this machine,
          // so the label the config file declared has to survive.
          urls: [
            {
              url: "https://smarthomeu.ddev.site",
              environment: "local",
              source: ".ddev/config.yaml",
            },
          ],
        };
      }
      if (command === "add_project") return 21;
      return null;
    });

    render(<AddProjectForm onCreated={vi.fn()} onCancel={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /select folder/i }));

    await waitFor(() => {
      expect(screen.getByText("/tmp/smarthomeu")).toBeInTheDocument();
    });

    expect((screen.getByLabelText("Site URL") as HTMLInputElement).value).toBe(
      "https://smarthomeu.ddev.site",
    );
    expect((screen.getByDisplayValue("Local") as HTMLSelectElement).value).toBe("local");

    fireEvent.click(screen.getByRole("button", { name: /create project/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "add_project",
        expect.objectContaining({
          urls: [{ url: "https://smarthomeu.ddev.site", environment: "local", source: "manual" }],
        }),
      );
    });
  });

  it("creates a code-only project when detection finds only a dev-server URL", async () => {
    openMock.mockResolvedValue("/tmp/sitecmd-devonly");
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_project_urls") {
        return {
          path: "/tmp/sitecmd-devonly",
          name: "sitecmd-devonly",
          framework: "Django",
          urls: [{ url: "http://127.0.0.1:8000", environment: "local", source: "manage.py" }],
        };
      }
      if (command === "add_project") return 12;
      return null;
    });

    render(<AddProjectForm onCreated={vi.fn()} onCancel={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /select folder/i }));

    await waitFor(() => {
      expect(screen.getByText("/tmp/sitecmd-devonly")).toBeInTheDocument();
    });

    expect((screen.getByLabelText("Site URL") as HTMLInputElement).value).toBe("");
    expect(screen.getByText(/No environment URL was detected/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /create project/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("add_project", expect.objectContaining({ urls: [] }));
    });
  });

  it("keeps the user-entered localhost URL primary when detected loopback aliases overlap", async () => {
    openMock.mockResolvedValue("/tmp/sitecmd-marketing");
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_project_urls") {
        return {
          path: "/tmp/sitecmd-marketing",
          name: "sitecmd-marketing",
          framework: "Astro",
          urls: [
            {
              url: "http://127.0.0.1:4321/",
              environment: "production",
              source: "package.json",
            },
          ],
        };
      }
      if (command === "add_project") return 42;
      return null;
    });
    const onCreated = vi.fn();

    render(<AddProjectForm onCreated={onCreated} onCancel={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Site URL"), {
      target: { value: "http://localhost:4321/" },
    });
    expect((screen.getByDisplayValue("Local") as HTMLSelectElement).value).toBe("local");

    fireEvent.click(screen.getByRole("button", { name: /select folder/i }));

    await waitFor(() => {
      expect(screen.getByText("/tmp/sitecmd-marketing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: /create project/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "add_project",
        expect.objectContaining({
          urls: [
            {
              // normalizeProjectUrlInput strips the trailing slash the user typed.
              url: "http://localhost:4321",
              environment: "local",
              source: "manual",
            },
          ],
        }),
      );
    });
    expect(onCreated).toHaveBeenCalledWith(42);
  });

  it("says why a URL-only creation failed instead of appearing to do nothing", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "add_project_by_url") throw new Error("Could not resolve that host");
      return null;
    });
    const onCreated = vi.fn();

    render(<AddProjectForm onCreated={onCreated} onCancel={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "Broken" } });
    fireEvent.change(screen.getByLabelText("Site URL"), { target: { value: "example.com" } });
    fireEvent.click(screen.getByRole("button", { name: /create project/i }));

    // URL mode had no error branch: the click completed, the dialog stayed
    // open, and nothing was shown. Indistinguishable from a dead button.
    expect(await screen.findByRole("alert")).toHaveTextContent(/Could not resolve that host/i);
    expect(onCreated).not.toHaveBeenCalled();
  });

  it("hands back a project that was created before a later step failed", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "add_project_by_url") return 91;
      if (command === "add_environment_url") throw new Error("That URL is not reachable");
      return null;
    });
    const onCreated = vi.fn();

    render(<AddProjectForm onCreated={onCreated} onCancel={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "Partial" } });
    fireEvent.change(screen.getByLabelText("Site URL"), { target: { value: "example.com" } });
    fireEvent.click(screen.getByRole("button", { name: /add environment/i }));
    fireEvent.change(screen.getByLabelText("Environment 2 URL"), {
      target: { value: "staging.example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: /create project/i }));

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(91));
  });
});
