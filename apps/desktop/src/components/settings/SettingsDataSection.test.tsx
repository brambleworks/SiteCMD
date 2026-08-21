import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn<(...args: unknown[]) => Promise<unknown>>(() => Promise.resolve(null)),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(() => Promise.resolve(null)),
  open: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ success: vi.fn(), error: vi.fn() }),
}));

import { DataSection, DeleteProjectCard } from "./SettingsDataSection";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

describe("DataSection cache freshness", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("shows cached database info while revalidating runtime size on remount", async () => {
    let sizeReads = 0;
    invokeMock.mockImplementation(async (command: unknown) => {
      if (command === "get_db_path") return "/tmp/sitecmd.db";
      if (command === "get_db_size") {
        sizeReads += 1;
        return sizeReads === 1 ? 1024 : 2048;
      }
      return null;
    });
    const queryClient = createTestQueryClient();
    const first = render(<DataSection view="data" />, {
      wrapper: withQueryClient(queryClient),
    });
    expect(await screen.findByText("1.0 KB")).toBeInTheDocument();
    first.unmount();

    render(<DataSection view="data" />, { wrapper: withQueryClient(queryClient) });

    expect(screen.getByText("1.0 KB")).toBeInTheDocument();
    expect(await screen.findByText("2.0 KB")).toBeInTheDocument();
    expect(sizeReads).toBe(2);
  });
});

describe("DeleteProjectCard", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
  });

  it("requires typing the project name before the delete button arms", () => {
    render(<DeleteProjectCard projectId={4} projectName="example.com" />);

    fireEvent.click(screen.getByRole("button", { name: /delete project/i }));

    const confirmInput = screen.getByLabelText(/to confirm/i);
    const deleteButton = screen.getByRole("button", { name: "Delete permanently" });
    expect(deleteButton).toBeDisabled();

    fireEvent.change(confirmInput, { target: { value: "other-project.com" } });
    expect(deleteButton).toBeDisabled();

    fireEvent.click(deleteButton);
    expect(invokeMock).not.toHaveBeenCalledWith("delete_project", expect.anything());

    fireEvent.change(confirmInput, { target: { value: "example.com" } });
    expect(deleteButton).toBeEnabled();
  });

  it("deletes and notifies the shell once the typed name matches", async () => {
    const onProjectDeleted = vi.fn(() => Promise.resolve());
    render(
      <DeleteProjectCard
        projectId={4}
        projectName="example.com"
        onProjectDeleted={onProjectDeleted}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /delete project/i }));
    fireEvent.change(screen.getByLabelText(/to confirm/i), {
      target: { value: " example.com " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Delete permanently" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("delete_project", { projectId: 4 });
      expect(onProjectDeleted).toHaveBeenCalledTimes(1);
    });
  });

  it("does not delete when Enter is pressed on a non-matching name", () => {
    render(<DeleteProjectCard projectId={4} projectName="example.com" />);

    fireEvent.click(screen.getByRole("button", { name: /delete project/i }));
    const confirmInput = screen.getByLabelText(/to confirm/i);
    fireEvent.change(confirmInput, { target: { value: "example" } });
    fireEvent.keyDown(confirmInput, { key: "Enter" });

    expect(invokeMock).not.toHaveBeenCalledWith("delete_project", expect.anything());
  });

  it("cancelling clears the typed name so reopening starts disarmed", () => {
    render(<DeleteProjectCard projectId={4} projectName="example.com" />);

    fireEvent.click(screen.getByRole("button", { name: /delete project/i }));
    fireEvent.change(screen.getByLabelText(/to confirm/i), {
      target: { value: "example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    fireEvent.click(screen.getByRole("button", { name: /delete project/i }));
    expect(screen.getByLabelText(/to confirm/i)).toHaveValue("");
    expect(screen.getByRole("button", { name: "Delete permanently" })).toBeDisabled();
  });
});
