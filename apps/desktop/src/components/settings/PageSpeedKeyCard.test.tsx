import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ success: vi.fn(), error: vi.fn(), info: vi.fn() }),
}));

import { PageSpeedKeyCard } from "./PageSpeedKeyCard";
import { withQueryClient } from "@/test-utils/query-client";

function renderPageSpeed(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

describe("PageSpeedKeyCard", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("saves a trimmed key when none is stored yet", async () => {
    invokeMock.mockImplementation((cmd: string) =>
      Promise.resolve(cmd === "pagespeed_api_key_is_set" ? false : undefined),
    );
    renderPageSpeed(<PageSpeedKeyCard />);

    fireEvent.change(await screen.findByLabelText("API key"), {
      target: { value: "  my-psi-key  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save key" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_pagespeed_api_key", { key: "my-psi-key" }),
    );
  });

  it("discloses that the key is sent to Google, not that it never leaves the machine", () => {
    invokeMock.mockImplementation((cmd: string) =>
      Promise.resolve(cmd === "pagespeed_api_key_is_set" ? false : undefined),
    );
    const { container } = renderPageSpeed(<PageSpeedKeyCard />);
    const text = container.textContent ?? "";
    expect(text).not.toContain("never leaves your machine");
    expect(text).toContain("sent only to Google");
    expect(text).toContain("never to SiteCMD");
  });

  it("offers Remove (clears the key) when one is already stored", async () => {
    invokeMock.mockImplementation((cmd: string) =>
      Promise.resolve(cmd === "pagespeed_api_key_is_set" ? true : undefined),
    );
    renderPageSpeed(<PageSpeedKeyCard />);

    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_pagespeed_api_key", { key: "" }),
    );
  });
});
